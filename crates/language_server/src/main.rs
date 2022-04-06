use std::collections::HashMap;

use tokio::sync::RwLock;
use tower_lsp::{LspService, Server};

mod file;
mod model;
mod project;
mod server;

use crate::server::Backend;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        models: RwLock::new(Vec::new()),
    })
    .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}
