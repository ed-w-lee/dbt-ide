use std::path::PathBuf;

use dashmap::DashMap;
use tower_lsp::{
    jsonrpc::Error,
    lsp_types::{
        CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
        DidCloseTextDocumentParams, DidOpenTextDocumentParams, InitializeParams, InitializeResult,
        MessageType, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
    },
    Client, LanguageServer,
};

use crate::{
    project::DbtProject,
    utils::{read_file, uri_to_path},
};

type JsonRpcResult<T> = tower_lsp::jsonrpc::Result<T>;

pub struct Backend {
    pub client: Client,
    pub projects: DashMap<PathBuf, DbtProject>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> JsonRpcResult<InitializeResult> {
        let root_uri = match params.root_uri {
            None => return Err(Error::invalid_params("language server requires root uri")),
            Some(uri) => uri,
        };
        let root_path = uri_to_path(&root_uri)?;
        let project = match DbtProject::find_single_project(&root_path).await {
            Err(msg) => {
                return Err(Error::invalid_params(format!(
                    "language server requires dbt_project.yml to exist in path: {:?}",
                    msg
                )))
            }
            Ok(project) => project,
        };
        self.projects.insert(root_path.to_path_buf(), project);

        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec!["(".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                }),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn shutdown(&self) -> JsonRpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let path = match uri_to_path(&params.text_document.uri) {
            Ok(path) => path,
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!(
                            "couldn't parse uri={:?} to path due to {:?}",
                            params.text_document.uri, e
                        ),
                    )
                    .await;
                return;
            }
        };
        let file_contents = match read_file(&path).await {
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("failed to read newly opened file {e}"),
                    )
                    .await;
                return;
            }
            Ok(contents) => contents,
        };
        for project in self.projects.iter() {
            if path.starts_with(project.key()) {
                match project.on_file_open(&path, &file_contents) {
                    Ok(_) => (),
                    Err(e) => {
                        self.client
                            .log_message(
                                MessageType::ERROR,
                                format!("failed to handle newly opened file correctly - {e}"),
                            )
                            .await;
                    }
                }
            }
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let path = match uri_to_path(&params.text_document.uri) {
            Ok(path) => path,
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!(
                            "couldn't parse uri={:?} to path due to {:?}",
                            params.text_document.uri, e
                        ),
                    )
                    .await;
                return;
            }
        };
        let file_contents = &params.content_changes[0].text;
        for project in self.projects.iter() {
            if path.starts_with(project.key()) {
                eprintln!("updating parse");
                match project.on_file_change(&path, file_contents) {
                    Ok(_) => (),
                    Err(e) => {
                        self.client
                            .log_message(
                                MessageType::ERROR,
                                format!("failed to handle changed file correctly - {e}"),
                            )
                            .await;
                    }
                }
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let path = match uri_to_path(&params.text_document.uri) {
            Ok(path) => path,
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!(
                            "couldn't parse uri={:?} to path due to {:?}",
                            params.text_document.uri, e
                        ),
                    )
                    .await;
                return;
            }
        };
        let file_contents = match read_file(&path).await {
            Err(e) => {
                if path.exists() {
                    self.client
                        .log_message(
                            MessageType::ERROR,
                            format!(
                                "failed to read existing file path={:?} due to {:?}",
                                params.text_document.uri, e
                            ),
                        )
                        .await;
                    return;
                }
                None
            }
            Ok(contents) => Some(contents),
        };
        for project in self.projects.iter() {
            if path.starts_with(project.key()) {
                match project.on_file_close(path.clone(), &file_contents) {
                    Ok(_) => (),
                    Err(e) => {
                        self.client
                            .log_message(
                                MessageType::ERROR,
                                format!("failed to handle closed file correctly - {e}"),
                            )
                            .await;
                    }
                }
            }
        }
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> JsonRpcResult<Option<CompletionResponse>> {
        let current_uri = params.text_document_position.text_document.uri;
        let path = match uri_to_path(&current_uri) {
            Ok(path) => path,
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!(
                            "couldn't open file with uri={:?} due to {:?}",
                            current_uri, e
                        ),
                    )
                    .await;
                return Err(Error::parse_error());
            }
        };

        Ok(Some(CompletionResponse::Array(
            self.projects
                .iter()
                .filter_map(|project| {
                    if path.starts_with(project.key()) {
                        Some(project.get_completion_items(
                            path.clone(),
                            params.text_document_position.position,
                        ))
                    } else {
                        None
                    }
                })
                .flatten()
                .collect(),
        )))
    }
}
