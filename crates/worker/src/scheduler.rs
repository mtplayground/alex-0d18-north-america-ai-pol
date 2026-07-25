//! Cadence-driven worker scheduling.

use std::{error::Error, time::Duration};

use crate::orchestration::IngestionOrchestrator;

/// Runs source ingestion immediately and then at a configured cadence.
pub struct Scheduler {
    cadence: Duration,
}

impl Scheduler {
    /// Creates a scheduler with the supplied delay between ingestion passes.
    pub const fn new(cadence: Duration) -> Self {
        Self { cadence }
    }

    /// Continues ingestion until the process receives a shutdown signal.
    pub async fn run(
        &self,
        orchestrator: &IngestionOrchestrator,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        loop {
            orchestrator.run_once().await?;
            println!("next ingestion pass in {} seconds", self.cadence.as_secs());

            tokio::select! {
                () = tokio::time::sleep(self.cadence) => {}
                signal = tokio::signal::ctrl_c() => {
                    signal?;
                    println!("worker received shutdown signal");
                    return Ok(());
                }
            }
        }
    }
}
