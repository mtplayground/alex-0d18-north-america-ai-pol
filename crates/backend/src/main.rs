//! HTTP server entry point.
//!
//! API routes, configuration, persistence, and static frontend delivery are added
//! in their dedicated issues. This executable establishes the Axum application
//! boundary and the network convention used by local and hosted deployments.

use std::error::Error;

use axum::{routing::get, Router};
use policy_shared::BackendConfig;

mod db;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = BackendConfig::from_env()?;
    let database = db::Database::connect(&config.database_url).await?;
    let app = Router::new().route("/", get(root)).with_state(database);
    let listener = tokio::net::TcpListener::bind(config.listen_address).await?;

    println!("database migrations applied");
    println!("backend listening on http://{}", config.listen_address);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn root() -> &'static str {
    "backend workspace initialized"
}
