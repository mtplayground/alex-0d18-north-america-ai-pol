//! DTOs for an individual policy entry and its observed version history.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde_json::Value;

use crate::PolicyChangeKind;

/// The current data for one policy entry.
#[derive(Clone, Debug, Serialize)]
pub struct EntryDetail {
    pub id: i64,
    pub source_id: i64,
    pub source_external_id: String,
    pub title: String,
    pub region: String,
    /// Category of the source that published this entry.
    pub source_category: String,
    pub agency: String,
    pub publication_date: Option<NaiveDate>,
    pub status: String,
    pub source_url: String,
}

/// One immutable state in an entry's history.
#[derive(Clone, Debug, Serialize)]
pub struct EntryVersionDetail {
    pub id: i64,
    pub version_number: i32,
    pub change_kind: PolicyChangeKind,
    /// Canonical source fields retained for a future full-diff experience.
    pub canonical_content: Value,
    pub content_hash: String,
    pub observed_at: DateTime<Utc>,
    pub change_summary: Option<String>,
}

/// Detail response for an expandable entry view.
#[derive(Clone, Debug, Serialize)]
pub struct EntryDetailResponse {
    pub entry: EntryDetail,
    /// Versions are ordered from newest to oldest.
    pub versions: Vec<EntryVersionDetail>,
}
