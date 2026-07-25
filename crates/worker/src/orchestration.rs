//! Source iteration and ordered ingestion pipeline boundary.

use std::error::Error;

use policy_shared::{
    NormalizationError, NormalizedPolicyRecord, PolicyNormalizer, Region, Source, SourceDocument,
};
use sha2::Digest;
use sqlx::{postgres::PgPool, FromRow};

use crate::{fetcher::SourceFetcher, storage::SnapshotStorage};

/// Executes one pass over every enabled source.
pub struct IngestionOrchestrator {
    database: PgPool,
    fetcher: SourceFetcher,
    normalizers: Vec<Box<dyn PolicyNormalizer>>,
    snapshot_storage: SnapshotStorage,
}

impl IngestionOrchestrator {
    /// Creates a source ingestion orchestrator.
    pub fn new(
        database: PgPool,
        fetcher: SourceFetcher,
        normalizers: Vec<Box<dyn PolicyNormalizer>>,
        snapshot_storage: SnapshotStorage,
    ) -> Self {
        Self {
            database,
            fetcher,
            normalizers,
            snapshot_storage,
        }
    }

    /// Fetches the enabled sources and processes them independently.
    pub async fn run_once(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        for source in enabled_sources(&self.database).await? {
            if let Err(error) = self.process_source(&source).await {
                eprintln!("source {} failed: {error}", source.id);
            }
        }

        Ok(())
    }

    async fn process_source(
        &self,
        source: &Source,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let fetched = self.fetch(source).await?;
        let normalized = self.normalize(fetched)?;

        if self.detect_change(&normalized) {
            self.store(normalized);
        }

        Ok(())
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

    fn detect_change(&self, document: &NormalizedDocument) -> bool {
        println!(
            "detecting changes for source {} ({} records from {} bytes, hash {})",
            document.source_id,
            document.records.len(),
            document.raw_size,
            document.content_hash
        );
        true
    }

    fn store(&self, document: NormalizedDocument) {
        println!(
            "prepared {} records from source {} with snapshot {} ({})",
            document.records.len(),
            document.source_id,
            document.raw_snapshot_key,
            document.content_type
        );
    }
}

#[derive(FromRow)]
struct SourceRow {
    id: i64,
    region: String,
    agency: String,
    base_url: String,
    crawl_config: serde_json::Value,
    enabled: bool,
}

async fn enabled_sources(database: &PgPool) -> Result<Vec<Source>, sqlx::Error> {
    let rows = sqlx::query_as::<_, SourceRow>(
        "SELECT id, region, agency, base_url, crawl_config, enabled FROM sources WHERE enabled ORDER BY id",
    )
    .fetch_all(database)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Source {
            id: row.id,
            region: if row.region == "us" {
                Region::UnitedStates
            } else {
                Region::Canada
            },
            agency: row.agency,
            base_url: row.base_url,
            crawl_config: row.crawl_config,
            enabled: row.enabled,
        })
        .collect())
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
