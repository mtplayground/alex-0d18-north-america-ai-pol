//! Shared, infrastructure-independent contracts for the API and ingestion worker.
//!
//! Persistence, HTTP, and vendor integrations intentionally live in their owning
//! crates. This crate is the single home for domain values used across boundaries.

pub mod config;
pub mod domain;

pub use config::{BackendConfig, ConfigError, WorkerConfig};
pub use domain::Region;
