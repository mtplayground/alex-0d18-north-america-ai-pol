//! Contracts for policy and news sources tracked by ingestion.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Region, SourceCategory};

/// A configured website or publication tracked for updates.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Source {
    /// Database identifier assigned when the source is created.
    pub id: i64,
    /// Geographic jurisdiction of the publishing government.
    pub region: Region,
    /// Whether the source provides government policy or AI news.
    pub category: SourceCategory,
    /// Government organization responsible for the source.
    pub agency: String,
    /// Canonical base URL used to fetch source material.
    pub base_url: String,
    /// Source-specific crawler settings retained as structured JSON.
    pub crawl_config: Value,
    /// Whether scheduled ingestion should include this source.
    pub enabled: bool,
}
