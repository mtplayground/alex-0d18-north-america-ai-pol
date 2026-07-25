//! Contracts for government sources tracked by ingestion.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Region;

/// A government website or publication tracked for policy updates.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Source {
    /// Database identifier assigned when the source is created.
    pub id: i64,
    /// Geographic jurisdiction of the publishing government.
    pub region: Region,
    /// Government organization responsible for the source.
    pub agency: String,
    /// Canonical base URL used to fetch source material.
    pub base_url: String,
    /// Source-specific crawler settings retained as structured JSON.
    pub crawl_config: Value,
    /// Whether scheduled ingestion should include this source.
    pub enabled: bool,
}
