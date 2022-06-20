use std::io;

use dashmap::DashMap;
use tower_lsp::{LspService, Server};

mod entity;
mod files;
mod position_finder;
mod server;
mod utils;

use crate::server::Backend;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "dbt_language_server=debug,tower_http=debug".into()),
        ))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(io::stderr)
                .with_ansi(false),
        )
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        projects: DashMap::new(),
    })
    .finish();

    tracing::debug!("built lsp service");

    Server::new(stdin, stdout, socket).serve(service).await;
}
