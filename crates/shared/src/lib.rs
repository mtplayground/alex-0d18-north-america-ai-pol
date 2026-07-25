//! Shared, infrastructure-independent contracts for the API and ingestion worker.
//!
//! Persistence, HTTP, and vendor integrations intentionally live in their owning
//! crates. This crate is the single home for domain values used across boundaries.

pub mod change_feed;
pub mod config;
pub mod domain;
pub mod normalization;
pub mod policy_entry;
pub mod policy_version;
pub mod source;

pub use config::{BackendConfig, ConfigError, WorkerConfig};
pub use change_feed::{ChangeFeedItem, ChangeFeedQuery, ChangeFeedResponse};
pub use domain::Region;
pub use normalization::{
    NormalizationError, NormalizedPolicyRecord, PolicyNormalizer, SourceDocument,
};
pub use policy_entry::PolicyEntry;
pub use policy_version::{PolicyChangeKind, PolicyVersion};
pub use source::Source;
