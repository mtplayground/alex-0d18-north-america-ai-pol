//! Source iteration and ordered ingestion pipeline boundary.

use std::error::Error;

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
        let fetched = self.fetch(source).await?;
        let normalized = self.normalize(fetched)?;

        let outcome = self.detect_changes(&normalized).await?;
        self.store(&normalized, &outcome);

        Ok(SourceProcessOutcome {
            raw_snapshot_key: normalized.raw_snapshot_key,
            records_processed: normalized.records.len(),
            change_detection: outcome,
        })
    }

    async fn fetch(
        &self,
        source: &Source,
    ) -> Result<FetchedDocument, Box<dyn Error + Send + Sync>> {
        println!("fetching source {} ({})", source.id, source.base_url);
        let payload = self.fetcher.fetch(source).await?;
        let content_hash = format!("{:x}", sha2::Sha256::digest(&payload.body));
        let snapshot = self
            .snapshot_storage
            .store(
                source.id,
                &content_hash,
                &payload.content_type,
                payload.body.clone(),
            )
            .await?;

        println!("captured {} at {}", snapshot.object_key, payload.url);
        Ok(FetchedDocument {
            source_id: source.id,
            source: source.clone(),
            source_url: payload.url.to_string(),
            content_hash,
            content_type: payload.content_type,
            raw_document: payload.body,
            raw_snapshot_key: snapshot.object_key,
        })
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

struct SourceProcessOutcome {
    raw_snapshot_key: String,
    records_processed: usize,
    change_detection: ChangeDetectionOutcome,
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
