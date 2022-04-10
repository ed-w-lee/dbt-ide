use std::collections::HashMap;

use dashmap::DashMap;
use sql_file::ModelFile;
use tokio::sync::RwLock;
use tower_lsp::{LspService, Server};

mod model;
mod position_finder;
mod project;
mod project_spec;
mod server;
mod sql_file;
mod utils;

use crate::server::Backend;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        project: RwLock::new(None),
        models: DashMap::new(),
        macros: DashMap::new(),
    })
    .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}
