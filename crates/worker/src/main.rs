//! Ingestion process entry point.
//!
//! Scheduling and ingestion orchestration are intentionally introduced in later
//! issues; this executable verifies the worker has an independent runtime target.

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("worker workspace initialized");
    tokio::signal::ctrl_c().await?;
    println!("worker received shutdown signal");
    Ok(())
}
