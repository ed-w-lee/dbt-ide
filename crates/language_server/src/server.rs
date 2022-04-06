use std::collections::HashMap;

use tokio::sync::RwLock;
use tower_lsp::{
    jsonrpc::{Error, Result},
    lsp_types::{
        CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams,
        CompletionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
        DidOpenTextDocumentParams, InitializeParams, InitializeResult, MessageType,
        ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
    },
    Client, LanguageServer,
};

use crate::{file::SqlFile, project::find_dbt_project};

#[derive(Debug)]
pub struct Backend {
    pub client: Client,
    pub models: RwLock<Vec<String>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let root_uri = params.root_uri;
        if root_uri.is_none() {
            return Err(Error::invalid_params("language server requires root uri"));
        }
        let root_uri = root_uri.unwrap().to_file_path();
        if root_uri.is_err() {
            return Err(Error::invalid_params(
                "language server needs to run locally",
            ));
        }
        let root_path = root_uri.unwrap();
        let project = match find_dbt_project(&root_path) {
            Err(msg) => {
                return Err(Error::invalid_params(format!(
                    "language server requires dbt_project.yml to exist in path: {:?}",
                    msg
                )))
            }
            Ok(project) => project,
        };
        let found_model_paths = project.get_model_paths();
        let found_models = found_model_paths.iter().map(|model_path| {
            model_path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string()
        });
        {
            let mut models = self.models.write().await;
            models.clear();
            models.extend(found_models);
        }
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: None,
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                }),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, format!("did_open: {:?}", params))
            .await;
        let uri = params.text_document.uri;
        let document = params.text_document.text;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, format!("did_change: {:?}", params))
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, format!("did_close: {:?}", params))
            .await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let models = self.models.read().await;
        Ok(Some(CompletionResponse::Array(
            models
                .iter()
                .map(|model| CompletionItem {
                    label: model.to_owned(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    ..Default::default()
                })
                .collect(),
        )))
    }
}
