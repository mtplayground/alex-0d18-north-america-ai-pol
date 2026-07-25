//! HTTP server entry point.
//!
//! API routes, configuration, persistence, and static frontend delivery are added
//! in their dedicated issues. This executable establishes the Axum application
//! boundary and the network convention used by local and hosted deployments.

use std::error::Error;

use axum::{routing::get, Json, Router};
use policy_shared::BackendConfig;
use serde::Serialize;
use tower_http::services::{ServeDir, ServeFile};

mod db;
mod detail;
mod feed;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = BackendConfig::from_env()?;
    let database = db::Database::connect(&config.database_url).await?;
    let frontend_dist_dir = config.frontend_dist_dir;
    let frontend_index = format!("{frontend_dist_dir}/index.html");
    let static_files =
        ServeDir::new(frontend_dist_dir).not_found_service(ServeFile::new(frontend_index));
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/changes", get(feed::list_changes))
        .route("/api/entries/{id}", get(detail::get_entry))
        .fallback_service(static_files)
        .with_state(database);
    let listener = tokio::net::TcpListener::bind(config.listen_address).await?;

    println!("database migrations applied");
    println!("backend listening on http://{}", config.listen_address);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[cfg(test)]
mod e2e_tests {
    use std::{env, time::Duration};

    use axum::{
        extract::{Query, State},
        http::header::CONTENT_TYPE,
        response::IntoResponse,
        routing::{get, post},
        Json, Router,
    };
    use policy_shared::{
        config::AiSummarizerConfig, ChangeFeedQuery, PolicyNormalizer, Region, Source,
        SourceDocument,
    };
    use policy_worker::{
        change_detection::ChangeDetector, fetcher::SourceFetcher, summarizer::AiSummarizer,
        us_normalizer::UsGovernmentNormalizer,
    };
    use serde_json::{json, Value};
    use sqlx::PgPool;
    use tokio::net::TcpListener;

    use super::{db::Database, feed};

    const FIXTURE_SOURCE: &str =
        include_str!("../../../tests/fixtures/federal-register-ai-policy.json");

    #[tokio::test]
    #[ignore = "requires an isolated PostgreSQL URL in E2E_DATABASE_URL"]
    async fn fixture_source_flows_from_ingestion_to_feed() {
        let database_url = env::var("E2E_DATABASE_URL")
            .expect("set E2E_DATABASE_URL to an isolated PostgreSQL database");
        let fixture_base_url = start_fixture_server().await;
        let database = Database::connect(&database_url).await.unwrap();
        let agency = format!(
            "E2E Fixture Policy Agency {}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let source_id = create_source(&database.pool, &fixture_base_url, &agency).await;
        let source = Source {
            id: source_id,
            region: Region::UnitedStates,
            agency: agency.clone(),
            base_url: fixture_base_url.clone(),
            crawl_config: json!({ "start_paths": ["/fixture/policy-feed.json"] }),
            enabled: true,
        };

        let fetcher = SourceFetcher::new().unwrap();
        let fetched = fetcher.fetch(&source).await.unwrap();
        let records = UsGovernmentNormalizer
            .normalize(
                &source,
                SourceDocument {
                    source_url: fetched.url.as_str(),
                    content_type: &fetched.content_type,
                    body: &fetched.body,
                },
            )
            .unwrap();
        let summarizer = AiSummarizer::from_config(&AiSummarizerConfig {
            api_key: "e2e-fixture-key".to_owned(),
            base_url: format!("{fixture_base_url}/v1"),
            model: "e2e-fixture-model".to_owned(),
            request_timeout: Duration::from_secs(2),
        })
        .unwrap();
        let detector = ChangeDetector::new(database.pool.clone(), summarizer);
        let outcome = detector
            .detect_and_persist(source_id, &records, "e2e/raw-fixture.json")
            .await
            .unwrap();
        assert_eq!(outcome.new_entries, 1);

        let Json(feed_response) = feed::list_changes(
            State(database.clone()),
            Query(ChangeFeedQuery {
                agency: Some(agency.clone()),
                ..ChangeFeedQuery::default()
            }),
        )
        .await
        .unwrap();

        assert_eq!(feed_response.items.len(), 1);
        assert_eq!(feed_response.items[0].title, "Fixture AI Policy Update");
        assert_eq!(
            feed_response.items[0].change_summary.as_deref(),
            Some("Fixture summary of the policy change.")
        );

        cleanup_source(&database.pool, source_id).await;
    }

    async fn start_fixture_server() -> String {
        let app = Router::new()
            .route("/fixture/policy-feed.json", get(fixture_source))
            .route("/v1/messages", post(fixture_summary));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{address}")
    }

    async fn fixture_source() -> impl IntoResponse {
        ([(CONTENT_TYPE, "application/json")], FIXTURE_SOURCE)
    }

    async fn fixture_summary() -> Json<Value> {
        Json(json!({
            "content": [{
                "type": "text",
                "text": "Fixture summary of the policy change."
            }]
        }))
    }

    async fn create_source(pool: &PgPool, base_url: &str, agency: &str) -> i64 {
        sqlx::query_scalar(
            "INSERT INTO sources (region, agency, base_url, crawl_config) \
             VALUES ('us', $1, $2, $3) RETURNING id",
        )
        .bind(agency)
        .bind(base_url)
        .bind(json!({ "start_paths": ["/fixture/policy-feed.json"] }))
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn cleanup_source(pool: &PgPool, source_id: i64) {
        sqlx::query("DELETE FROM policy_entries WHERE source_id = $1")
            .bind(source_id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM sources WHERE id = $1")
            .bind(source_id)
            .execute(pool)
            .await
            .unwrap();
    }
}
