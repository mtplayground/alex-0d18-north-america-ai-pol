//! Ingestion process entry point.
//!
//! Scheduling and ingestion orchestration are intentionally introduced in later
//! issues; this executable verifies the worker has an independent runtime target.

use std::error::Error;

use policy_shared::{PolicyNormalizer, WorkerConfig};
use policy_worker::{
    ai_news_normalizer, canada_normalizer, fetcher, orchestration, scheduler, storage,
    summarizer, us_normalizer,
};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let config = WorkerConfig::from_env()?;
    let database = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;
    let snapshot_storage = storage::SnapshotStorage::from_config(&config.object_storage);
    let ai_summarizer = summarizer::AiSummarizer::from_optional_config(config.ai_summarizer.as_ref())?;
    let summaries_enabled = ai_summarizer.is_enabled();
    let fetcher = fetcher::SourceFetcher::new()?;
    let normalizers: Vec<Box<dyn PolicyNormalizer>> = vec![
        Box::new(ai_news_normalizer::AiNewsNormalizer),
        Box::new(us_normalizer::UsGovernmentNormalizer),
        Box::new(canada_normalizer::CanadaGovernmentNormalizer),
    ];
    let orchestrator = orchestration::IngestionOrchestrator::new(
        database,
        fetcher,
        normalizers,
        snapshot_storage,
        ai_summarizer,
    );
    let scheduler = scheduler::Scheduler::new(config.scheduler_cadence);

    if summaries_enabled {
        println!(
            "worker configured with object storage, AI summaries, and a {} second scheduler cadence",
            config.scheduler_cadence.as_secs()
        );
    } else {
        eprintln!(
            "AI summaries are disabled because AI_SUMMARIZER_API_KEY, AI_SUMMARIZER_BASE_URL, and AI_SUMMARIZER_MODEL are not all configured; ingestion will continue without summaries"
        );
        println!(
            "worker configured with object storage and a {} second scheduler cadence",
            config.scheduler_cadence.as_secs()
        );
    }
    scheduler.run(&orchestrator).await?;

    Ok(())
}
