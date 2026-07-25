//! Contracts for the current normalized state of a policy entry.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::Region;

/// A policy record discovered from a tracked government source.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PolicyEntry {
    /// Database identifier assigned when the entry is created.
    pub id: i64,
    /// The tracked government source that published this entry.
    pub source_id: i64,
    /// Human-readable policy title.
    pub title: String,
    /// Geographic jurisdiction for the policy.
    pub region: Region,
    /// Government organization responsible for the policy.
    pub agency: String,
    /// Publication or effective date supplied by the source, when known.
    pub publication_date: Option<NaiveDate>,
    /// Current source-provided policy status.
    pub status: String,
    /// Canonical source link for the policy entry.
    pub source_url: String,
}
