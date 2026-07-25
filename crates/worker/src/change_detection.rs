//! Transactional policy-entry change detection and version persistence.

use std::error::Error;

use policy_shared::NormalizedPolicyRecord;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPool, FromRow};

use crate::summarizer::AiSummarizer;

/// Counts produced by a change-detection pass.
#[derive(Default)]
pub struct ChangeDetectionOutcome {
    /// Records that created a policy entry and its initial version.
    pub new_entries: usize,
    /// Existing entries whose canonical content changed.
    pub updated_entries: usize,
    /// Existing entries whose canonical content was unchanged.
    pub unchanged_entries: usize,
}

/// Compares normalized records against the latest persisted version.
pub struct ChangeDetector {
    database: PgPool,
    summarizer: AiSummarizer,
}

impl ChangeDetector {
    /// Creates a detector backed by the worker's PostgreSQL pool.
    pub fn new(database: PgPool, summarizer: AiSummarizer) -> Self {
        Self {
            database,
            summarizer,
        }
    }

    /// Persists versions only for new or changed normalized records.
    pub async fn detect_and_persist(
        &self,
        source_id: i64,
        records: &[NormalizedPolicyRecord],
        raw_snapshot_key: &str,
    ) -> Result<ChangeDetectionOutcome, Box<dyn Error + Send + Sync>> {
        let mut outcome = ChangeDetectionOutcome::default();

        for record in records {
            match self.persist_record(source_id, record, raw_snapshot_key).await? {
                ChangeKind::New => outcome.new_entries += 1,
                ChangeKind::Updated => outcome.updated_entries += 1,
                ChangeKind::Unchanged => outcome.unchanged_entries += 1,
            }
        }

        Ok(outcome)
    }

    async fn persist_record(
        &self,
        source_id: i64,
        record: &NormalizedPolicyRecord,
        raw_snapshot_key: &str,
    ) -> Result<ChangeKind, Box<dyn Error + Send + Sync>> {
        let content_hash = canonical_hash(&record.canonical_content)?;
        let mut transaction = self.database.begin().await?;
        let entry_id = upsert_entry(&mut transaction, source_id, record).await?;
        let latest = sqlx::query_as::<_, LatestVersion>(
            "SELECT canonical_content, content_hash, version_number FROM policy_versions \
             WHERE policy_entry_id = $1 ORDER BY version_number DESC LIMIT 1",
        )
        .bind(entry_id)
        .fetch_optional(&mut *transaction)
        .await?;

        let change_kind = match latest {
            Some(version) if version.content_hash == content_hash => ChangeKind::Unchanged,
            Some(version) => {
                let summary = self
                    .summarize_change(record, Some(&version.canonical_content))
                    .await;
                update_entry(&mut transaction, entry_id, record).await?;
                insert_version(
                    &mut transaction,
                    entry_id,
                    version.version_number + 1,
                    "updated",
                    record,
                    &content_hash,
                    raw_snapshot_key,
                    summary.as_deref(),
                )
                .await?;
                ChangeKind::Updated
            }
            None => {
                let summary = self.summarize_change(record, None).await;
                insert_version(
                    &mut transaction,
                    entry_id,
                    1,
                    "new",
                    record,
                    &content_hash,
                    raw_snapshot_key,
                    summary.as_deref(),
                )
                .await?;
                ChangeKind::New
            }
        };

        transaction.commit().await?;
        Ok(change_kind)
    }

    async fn summarize_change(
        &self,
        record: &NormalizedPolicyRecord,
        previous_content: Option<&Value>,
    ) -> Option<String> {
        let prompt = summary_prompt(record, previous_content);

        match self.summarizer.summarize(&prompt).await {
            Ok(summary) => condense_summary(&summary),
            Err(error) => {
                eprintln!(
                    "AI summarization failed for source record {}: {error}",
                    record.source_external_id
                );
                None
            }
        }
    }
}

#[derive(FromRow)]
struct LatestVersion {
    canonical_content: Value,
    content_hash: String,
    version_number: i32,
}

enum ChangeKind {
    New,
    Updated,
    Unchanged,
}

fn canonical_hash(content: &Value) -> Result<String, serde_json::Error> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(content)?)))
}

fn summary_prompt(record: &NormalizedPolicyRecord, previous_content: Option<&Value>) -> String {
    let previous = previous_content
        .map(Value::to_string)
        .unwrap_or_else(|| "No prior version exists; explain the new policy entry.".to_owned());

    format!(
        "Write a factual one-to-two-line plain-language summary of the substantive policy change. \
         Do not use headings, bullets, or speculation.\n\nPolicy title: {}\nPrevious canonical content: {}\n\nCurrent canonical content: {}",
        record.title, previous, record.canonical_content
    )
}

fn condense_summary(summary: &str) -> Option<String> {
    let lines = summary
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(2)
        .collect::<Vec<_>>();

    (!lines.is_empty()).then(|| lines.join("\n"))
}

async fn upsert_entry(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_id: i64,
    record: &NormalizedPolicyRecord,
) -> Result<i64, sqlx::Error> {
    let entry = sqlx::query_as::<_, EntryId>(
        "INSERT INTO policy_entries \
         (source_id, source_external_id, title, region, agency, publication_date, status, source_url) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (source_id, source_external_id) \
         DO UPDATE SET updated_at = policy_entries.updated_at \
         RETURNING id",
    )
    .bind(source_id)
    .bind(&record.source_external_id)
    .bind(&record.title)
    .bind(record.region.as_code())
    .bind(&record.agency)
    .bind(record.publication_date)
    .bind(&record.status)
    .bind(&record.source_url)
    .fetch_one(&mut **transaction)
    .await?;

    Ok(entry.id)
}

async fn update_entry(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entry_id: i64,
    record: &NormalizedPolicyRecord,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE policy_entries SET title = $2, region = $3, agency = $4, publication_date = $5, \
         status = $6, source_url = $7, updated_at = NOW() WHERE id = $1",
    )
    .bind(entry_id)
    .bind(&record.title)
    .bind(record.region.as_code())
    .bind(&record.agency)
    .bind(record.publication_date)
    .bind(&record.status)
    .bind(&record.source_url)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

async fn insert_version(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entry_id: i64,
    version_number: i32,
    change_kind: &str,
    record: &NormalizedPolicyRecord,
    content_hash: &str,
    raw_snapshot_key: &str,
    change_summary: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO policy_versions \
         (policy_entry_id, version_number, change_kind, canonical_content, content_hash, raw_snapshot_key, change_summary) \
         VALUES ($1, $2, $3::policy_change_kind, $4, $5, $6, $7)",
    )
    .bind(entry_id)
    .bind(version_number)
    .bind(change_kind)
    .bind(&record.canonical_content)
    .bind(content_hash)
    .bind(raw_snapshot_key)
    .bind(change_summary)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

#[derive(FromRow)]
struct EntryId {
    id: i64,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{canonical_hash, condense_summary, summary_prompt};

    #[test]
    fn canonical_hash_is_stable_for_equivalent_content() {
        assert_eq!(
            canonical_hash(&json!({ "title": "Example", "status": "active" })).unwrap(),
            canonical_hash(&json!({ "status": "active", "title": "Example" })).unwrap(),
        );
    }

    #[test]
    fn summaries_are_limited_to_two_nonempty_lines() {
        assert_eq!(
            condense_summary("First line\n\nSecond line\nThird line"),
            Some("First line\nSecond line".to_owned())
        );
    }

    #[test]
    fn new_record_prompt_explains_that_no_prior_version_exists() {
        let record = policy_shared::NormalizedPolicyRecord {
            source_external_id: "example".to_owned(),
            title: "Example policy".to_owned(),
            region: policy_shared::Region::UnitedStates,
            agency: "Example agency".to_owned(),
            publication_date: None,
            status: "active".to_owned(),
            source_url: "https://example.test".to_owned(),
            canonical_content: json!({ "status": "active" }),
        };

        assert!(summary_prompt(&record, None).contains("No prior version exists"));
    }
}
