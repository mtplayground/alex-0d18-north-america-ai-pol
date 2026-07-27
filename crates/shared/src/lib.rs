//! Shared, infrastructure-independent contracts for the API and ingestion worker.
//!
//! Persistence, HTTP, and vendor integrations intentionally live in their owning
//! crates. This crate is the single home for domain values used across boundaries.

pub mod change_feed;
pub mod config;
pub mod crawl_config;
pub mod domain;
pub mod entry_detail;
pub mod normalization;
pub mod policy_entry;
pub mod policy_version;
pub mod source;

pub use config::{BackendConfig, ConfigError, WorkerConfig};
pub use crawl_config::{CrawlConfig, CrawlConfigError};
pub use change_feed::{ChangeFeedItem, ChangeFeedQuery, ChangeFeedResponse, ChangeFeedSort};
pub use domain::{Region, SourceCategory};
pub use entry_detail::{EntryDetail, EntryDetailResponse, EntryVersionDetail};
pub use normalization::{
    validate_record_quality, NormalizationError, NormalizedPolicyRecord, PolicyNormalizer,
    RecordQualityRejection, SourceDocument,
};
pub use policy_entry::PolicyEntry;
pub use policy_version::{PolicyChangeKind, PolicyVersion};
pub use source::Source;
