use dashmap::DashMap;
use tower_lsp::{LspService, Server};

mod files;
mod model;
mod position_finder;
mod project;
mod server;
mod utils;

use crate::server::Backend;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        projects: DashMap::new(),
    })
    .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}
