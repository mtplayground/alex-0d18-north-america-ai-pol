//! Ingestion process entry point.
//!
//! Scheduling and ingestion orchestration are intentionally introduced in later
//! issues; this executable verifies the worker has an independent runtime target.

use std::error::Error;

use policy_shared::WorkerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = WorkerConfig::from_env()?;
    println!(
        "worker configured with a {} second scheduler cadence",
        config.scheduler_cadence.as_secs()
    );
    tokio::signal::ctrl_c().await?;
    println!("worker received shutdown signal");
    Ok(())
}
