//! Immutable history contracts for observed policy changes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Classification assigned when an observed policy state is recorded.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyChangeKind {
    /// The first observed state for a policy entry.
    New,
    /// A later observed state with changed normalized content.
    Updated,
}

/// An immutable observed state in a policy entry's history.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PolicyVersion {
    /// Database identifier assigned when the version is created.
    pub id: i64,
    /// The policy entry whose history contains this version.
    pub policy_entry_id: i64,
    /// One-based sequence number within the policy entry's history.
    pub version_number: i32,
    /// Whether this is the initial observation or a subsequent update.
    pub change_kind: PolicyChangeKind,
    /// Canonical normalized state observed for this version.
    pub canonical_content: Value,
    /// Deterministic hash of the canonical normalized state.
    pub content_hash: String,
    /// Time at which this state was observed from its source.
    pub observed_at: DateTime<Utc>,
    /// Generated explanation of how this version differs from its predecessor.
    pub change_summary: Option<String>,
    /// Object-storage key for the raw source document used to create this version.
    pub raw_snapshot_key: Option<String>,
}
