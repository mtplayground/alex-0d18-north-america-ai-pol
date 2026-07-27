//! Source iteration and ordered ingestion pipeline boundary.

use std::{collections::HashSet, error::Error};

use policy_shared::{
    NormalizationError, NormalizedPolicyRecord, PolicyNormalizer, Region, Source, SourceCategory,
    SourceDocument,
};
use sha2::Digest;
use sqlx::{postgres::PgPool, FromRow};

use crate::{
    change_detection::{ChangeDetectionOutcome, ChangeDetector},
    fetcher::SourceFetcher,
    storage::SnapshotStorage,
    summarizer::AiSummarizer,
};

/// Executes one pass over every enabled source.
pub struct IngestionOrchestrator {
    database: PgPool,
    fetcher: SourceFetcher,
    normalizers: Vec<Box<dyn PolicyNormalizer>>,
    change_detector: ChangeDetector,
    snapshot_storage: SnapshotStorage,
}

impl IngestionOrchestrator {
    /// Creates a source ingestion orchestrator.
    pub fn new(
        database: PgPool,
        fetcher: SourceFetcher,
        normalizers: Vec<Box<dyn PolicyNormalizer>>,
        snapshot_storage: SnapshotStorage,
        ai_summarizer: AiSummarizer,
    ) -> Self {
        Self {
            change_detector: ChangeDetector::new(database.clone(), ai_summarizer),
            database,
            fetcher,
            normalizers,
            snapshot_storage,
        }
    }

    /// Fetches the enabled sources and processes them independently.
    pub async fn run_once(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        for source in enabled_sources(&self.database).await? {
            let run_id = match start_source_run(&self.database, source.id).await {
                Ok(run_id) => Some(run_id),
                Err(error) => {
                    eprintln!("could not record the start of source {}: {error}", source.id);
                    None
                }
            };

            match self.process_source(&source).await {
                Ok(outcome) => {
                    if let Some(run_id) = run_id {
                        if let Err(error) = finish_source_run(&self.database, run_id, &outcome).await
                        {
                            eprintln!("could not record success for source {}: {error}", source.id);
                        }
                    }
                }
                Err(error) => {
                    eprintln!("source {} failed: {error}", source.id);
                    if let Some(run_id) = run_id {
                        if let Err(record_error) = fail_source_run(&self.database, run_id, error.as_ref()).await
                        {
                            eprintln!("could not record failure for source {}: {record_error}", source.id);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_source(
        &self,
        source: &Source,
    ) -> Result<SourceProcessOutcome, Box<dyn Error + Send + Sync>> {
        let fetched_documents = self.fetch(source).await?;
        let mut source_outcome = SourceProcessOutcome::default();
        let mut deduplicator = RecordDeduplicator::default();

        for fetched_document in fetched_documents {
            let mut normalized = self.normalize(fetched_document)?;
            let duplicate_count =
                deduplicator.retain_unique(normalized.source_id, &mut normalized.records);
            if duplicate_count > 0 {
                println!(
                    "collapsed {duplicate_count} duplicate records from source {} before persistence",
                    normalized.source_id
                );
            }
            let change_detection = self.detect_changes(&normalized).await?;
            self.store(&normalized, &change_detection);
            source_outcome.record(normalized, change_detection);
        }

        Ok(source_outcome)
    }

    async fn fetch(
        &self,
        source: &Source,
    ) -> Result<Vec<FetchedDocument>, Box<dyn Error + Send + Sync>> {
        println!("fetching source {} ({})", source.id, source.base_url);
        let payloads = self.fetcher.fetch_all(source).await?;
        let mut documents = Vec::with_capacity(payloads.len());

        for payload in payloads {
            let content_hash = format!("{:x}", sha2::Sha256::digest(&payload.body));
            let source_url = payload.url.to_string();
            let snapshot = self
                .snapshot_storage
                .store(
                    source.id,
                    &source_url,
                    &content_hash,
                    &payload.content_type,
                    payload.body.clone(),
                )
                .await?;

            println!("captured {} at {}", snapshot.object_key, source_url);
            documents.push(FetchedDocument {
                source_id: source.id,
                source: source.clone(),
                source_url,
                content_hash,
                content_type: payload.content_type,
                raw_document: payload.body,
                raw_snapshot_key: snapshot.object_key,
            });
        }

        Ok(documents)
    }

    fn normalize(
        &self,
        document: FetchedDocument,
    ) -> Result<NormalizedDocument, NormalizationError> {
        println!("normalizing source {}", document.source_id);
        let raw_size = document.raw_document.len();
        let records = if let Some(normalizer) = self
            .normalizers
            .iter()
            .find(|normalizer| normalizer.supports(&document.source))
        {
            normalizer.normalize(
                &document.source,
                SourceDocument {
                    source_url: &document.source_url,
                    content_type: &document.content_type,
                    body: &document.raw_document,
                },
            )?
        } else {
            println!("no normalizer is configured for source {}", document.source_id);
            Vec::new()
        };

        Ok(NormalizedDocument {
            source_id: document.source_id,
            content_hash: document.content_hash,
            content_type: document.content_type,
            raw_size,
            raw_snapshot_key: document.raw_snapshot_key,
            records,
        })
    }

    async fn detect_changes(
        &self,
        document: &NormalizedDocument,
    ) -> Result<ChangeDetectionOutcome, Box<dyn Error + Send + Sync>> {
        println!(
            "detecting changes for source {} ({} records from {} bytes, hash {})",
            document.source_id,
            document.records.len(),
            document.raw_size,
            document.content_hash
        );
        self.change_detector
            .detect_and_persist(
                document.source_id,
                &document.records,
                &document.raw_snapshot_key,
            )
            .await
    }

    fn store(&self, document: &NormalizedDocument, outcome: &ChangeDetectionOutcome) {
        println!(
            "stored {} new, {} updated, and {} unchanged records from source {} with snapshot {} ({})",
            outcome.new_entries,
            outcome.updated_entries,
            outcome.unchanged_entries,
            document.source_id,
            document.raw_snapshot_key,
            document.content_type
        );
    }
}

#[derive(Default)]
struct SourceProcessOutcome {
    raw_snapshot_key: String,
    records_processed: usize,
    change_detection: ChangeDetectionOutcome,
}

impl SourceProcessOutcome {
    fn record(&mut self, document: NormalizedDocument, outcome: ChangeDetectionOutcome) {
        // A source-run row has a single audit-key column. Policy-version rows
        // retain the exact snapshot used for each document; use the final key
        // here as a concise pointer to this multi-path source run.
        self.raw_snapshot_key = document.raw_snapshot_key;
        self.records_processed = self.records_processed.saturating_add(document.records.len());
        self.change_detection.new_entries = self
            .change_detection
            .new_entries
            .saturating_add(outcome.new_entries);
        self.change_detection.updated_entries = self
            .change_detection
            .updated_entries
            .saturating_add(outcome.updated_entries);
        self.change_detection.unchanged_entries = self
            .change_detection
            .unchanged_entries
            .saturating_add(outcome.unchanged_entries);
    }
}

#[derive(FromRow)]
struct SourceRow {
    id: i64,
    region: String,
    category: String,
    agency: String,
    base_url: String,
    crawl_config: serde_json::Value,
    enabled: bool,
}

async fn enabled_sources(database: &PgPool) -> Result<Vec<Source>, sqlx::Error> {
    let rows = sqlx::query_as::<_, SourceRow>(
        "SELECT id, region, category, agency, base_url, crawl_config, enabled \
         FROM sources WHERE enabled ORDER BY id",
    )
    .fetch_all(database)
    .await?;

    rows.into_iter()
        .map(|row| {
            let region = Region::from_code(&row.region).ok_or_else(|| {
                sqlx::Error::Protocol(format!("unknown source region: {}", row.region))
            })?;
            let category = SourceCategory::from_code(&row.category).ok_or_else(|| {
                sqlx::Error::Protocol(format!("unknown source category: {}", row.category))
            })?;

            Ok(Source {
                id: row.id,
                region,
                category,
                agency: row.agency,
                base_url: row.base_url,
                crawl_config: row.crawl_config,
                enabled: row.enabled,
            })
        })
        .collect()
}

async fn start_source_run(database: &PgPool, source_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO source_ingestion_runs (source_id, status) VALUES ($1, 'running') RETURNING id",
    )
    .bind(source_id)
    .fetch_one(database)
    .await
}

async fn finish_source_run(
    database: &PgPool,
    run_id: i64,
    outcome: &SourceProcessOutcome,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE source_ingestion_runs SET status = 'succeeded', raw_snapshot_key = $2, \
         records_processed = $3, new_entries = $4, updated_entries = $5, unchanged_entries = $6, \
         completed_at = NOW() WHERE id = $1",
    )
    .bind(run_id)
    .bind(&outcome.raw_snapshot_key)
    .bind(i32::try_from(outcome.records_processed).unwrap_or(i32::MAX))
    .bind(i32::try_from(outcome.change_detection.new_entries).unwrap_or(i32::MAX))
    .bind(i32::try_from(outcome.change_detection.updated_entries).unwrap_or(i32::MAX))
    .bind(i32::try_from(outcome.change_detection.unchanged_entries).unwrap_or(i32::MAX))
    .execute(database)
    .await?;

    Ok(())
}

async fn fail_source_run(
    database: &PgPool,
    run_id: i64,
    error: &(dyn Error + Send + Sync),
) -> Result<(), sqlx::Error> {
    let message = error.to_string();
    let message = &message[..message.len().min(4_000)];
    sqlx::query(
        "UPDATE source_ingestion_runs SET status = 'failed', error_message = $2, completed_at = NOW() WHERE id = $1",
    )
    .bind(run_id)
    .bind(message)
    .execute(database)
    .await?;

    Ok(())
}

struct FetchedDocument {
    source_id: i64,
    source: Source,
    source_url: String,
    content_hash: String,
    content_type: String,
    raw_document: Vec<u8>,
    raw_snapshot_key: String,
}

struct NormalizedDocument {
    source_id: i64,
    content_hash: String,
    content_type: String,
    raw_size: usize,
    raw_snapshot_key: String,
    records: Vec<NormalizedPolicyRecord>,
}

/// Collapses repeated records across all fetched documents from one source run.
#[derive(Default)]
struct RecordDeduplicator {
    source_external_ids: HashSet<(i64, String)>,
    fallback_identities: HashSet<FallbackIdentity>,
}

#[derive(Eq, Hash, PartialEq)]
struct FallbackIdentity {
    title: String,
    agency: String,
    publication_date: chrono::NaiveDate,
}

impl RecordDeduplicator {
    /// Retains the first deterministic occurrence of every stable or fallback
    /// identity and returns the number of records removed.
    fn retain_unique(
        &mut self,
        source_id: i64,
        records: &mut Vec<NormalizedPolicyRecord>,
    ) -> usize {
        let mut unique_records = Vec::with_capacity(records.len());
        let mut duplicate_count = 0;

        for record in std::mem::take(records) {
            let source_external_id = stable_identity(source_id, &record);
            let fallback_identity = fallback_identity(&record);
            let repeated_external_id = source_external_id
                .as_ref()
                .is_some_and(|identity| self.source_external_ids.contains(identity));
            let repeated_fallback = fallback_identity
                .as_ref()
                .is_some_and(|identity| self.fallback_identities.contains(identity));

            if repeated_external_id || repeated_fallback {
                duplicate_count += 1;
                continue;
            }

            if let Some(identity) = source_external_id {
                self.source_external_ids.insert(identity);
            }
            if let Some(identity) = fallback_identity {
                self.fallback_identities.insert(identity);
            }
            unique_records.push(record);
        }

        *records = unique_records;
        duplicate_count
    }
}

fn stable_identity(source_id: i64, record: &NormalizedPolicyRecord) -> Option<(i64, String)> {
    let external_id = record.source_external_id.trim();
    (!external_id.is_empty()).then(|| (source_id, external_id.to_owned()))
}

fn fallback_identity(record: &NormalizedPolicyRecord) -> Option<FallbackIdentity> {
    let title = normalized_identity_component(&record.title);
    let agency = normalized_identity_component(&record.agency);

    match (title.is_empty(), agency.is_empty(), record.publication_date) {
        (false, false, Some(publication_date)) => Some(FallbackIdentity {
            title,
            agency,
            publication_date,
        }),
        _ => None,
    }
}

fn normalized_identity_component(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use policy_shared::{NormalizedPolicyRecord, Region};
    use serde_json::json;

    use super::RecordDeduplicator;

    fn record(
        external_id: &str,
        title: &str,
        publication_date: Option<NaiveDate>,
    ) -> NormalizedPolicyRecord {
        NormalizedPolicyRecord {
            source_external_id: external_id.to_owned(),
            title: title.to_owned(),
            region: Region::UnitedStates,
            agency: "Example Agency".to_owned(),
            publication_date,
            status: "active".to_owned(),
            source_url: format!("https://example.test/{external_id}"),
            canonical_content: json!({ "external_id": external_id }),
        }
    }

    #[test]
    fn collapses_repeated_source_external_ids_across_documents() {
        let mut deduplicator = RecordDeduplicator::default();
        let mut first_document = vec![record("policy-1", "Example policy", None)];
        let mut second_document = vec![record("policy-1", "Example policy", None)];

        assert_eq!(deduplicator.retain_unique(1, &mut first_document), 0);
        assert_eq!(deduplicator.retain_unique(1, &mut second_document), 1);
        assert_eq!(first_document.len(), 1);
        assert!(second_document.is_empty());
    }

    #[test]
    fn falls_back_to_title_agency_and_publication_date() {
        let publication_date = match NaiveDate::from_ymd_opt(2026, 7, 27) {
            Some(publication_date) => publication_date,
            None => panic!("valid date should construct"),
        };
        let mut deduplicator = RecordDeduplicator::default();
        let mut records = vec![
            record("policy-1", "  Example   policy ", Some(publication_date)),
            record("policy-2", "example policy", Some(publication_date)),
        ];

        assert_eq!(deduplicator.retain_unique(1, &mut records), 1);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source_external_id, "policy-1");
    }
}
