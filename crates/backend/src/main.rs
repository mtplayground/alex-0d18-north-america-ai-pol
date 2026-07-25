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
