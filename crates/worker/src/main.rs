//! Ingestion process entry point.
//!
//! Scheduling and ingestion orchestration are intentionally introduced in later
//! issues; this executable verifies the worker has an independent runtime target.

use std::error::Error;

use policy_shared::WorkerConfig;

pub mod storage;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = WorkerConfig::from_env()?;
    let _snapshot_storage = storage::SnapshotStorage::from_config(&config.object_storage);
    println!(
        "worker configured with object storage and a {} second scheduler cadence",
        config.scheduler_cadence.as_secs()
    );
    tokio::signal::ctrl_c().await?;
    println!("worker received shutdown signal");
    Ok(())
}
