//! HTTP server entry point.
//!
//! API routes, configuration, persistence, and static frontend delivery are added
//! in their dedicated issues. This executable establishes the Axum application
//! boundary and the network convention used by local and hosted deployments.

use std::{error::Error, net::SocketAddr};

use axum::{routing::get, Router};

const LISTEN_ADDRESS: &str = "0.0.0.0:8080";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app = Router::new().route("/", get(root));
    let address: SocketAddr = LISTEN_ADDRESS.parse()?;
    let listener = tokio::net::TcpListener::bind(address).await?;

    println!("backend listening on http://{address}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn root() -> &'static str {
    "backend workspace initialized"
}
