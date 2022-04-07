use std::collections::HashMap;

use dashmap::DashMap;
use sql_file::ModelFile;
use tower_lsp::{LspService, Server};

mod model;
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
        models: DashMap::new(),
    })
    .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}
