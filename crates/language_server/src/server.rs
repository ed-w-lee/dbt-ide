use std::path::PathBuf;

use dashmap::DashMap;
use tower_lsp::{
    jsonrpc::Error,
    lsp_types::{
        request::{GotoDeclarationParams, GotoDeclarationResponse},
        CompletionOptions, CompletionParams, CompletionResponse, DeclarationCapability,
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        InitializeParams, InitializeResult, MessageType, ServerCapabilities,
        TextDocumentSyncCapability, TextDocumentSyncKind, Url,
    },
    Client, LanguageServer,
};
use tracing::{event, field, info, Level};

use crate::{
    entity::DbtProject,
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
        tracing::debug!(message = "initializing");
        let root_uri = match params.root_uri {
            None => return Err(Error::invalid_params("language server requires root uri")),
            Some(uri) => uri,
        };
        let root_path = uri_to_path(&root_uri)?;
        let project = match DbtProject::find_single_project(&root_path).await {
            Err(msg) => {
                tracing::error!(message = "failed to find single project", ?root_path);
                return Err(Error::invalid_params(format!(
                    "language server requires dbt_project.yml to exist in path: {:?}",
                    msg
                )));
            }
            Ok(project) => project,
        };
        tracing::debug!(?root_path, ?project);
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
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn shutdown(&self) -> JsonRpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let path = match self.uri_to_path(&params.text_document.uri).await {
            Err(_) => return,
            Ok(path) => path,
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
        let path = match self.uri_to_path(&params.text_document.uri).await {
            Err(_) => return,
            Ok(path) => path,
        };
        let file_contents = &params.content_changes[0].text;
        for project in self.projects.iter() {
            if path.starts_with(project.key()) {
                tracing::info!(message="parsing project", project = ?project.key());
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
        let path = match self.uri_to_path(&params.text_document.uri).await {
            Err(_) => return,
            Ok(path) => path,
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
        let path = self.uri_to_path(&current_uri).await?;

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

    async fn goto_declaration(
        &self,
        params: GotoDeclarationParams,
    ) -> JsonRpcResult<Option<GotoDeclarationResponse>> {
        let current_uri = params.text_document_position_params.text_document.uri;
        let path = self.uri_to_path(&current_uri).await?;
        Ok(Some(GotoDeclarationResponse::Link(
            self.projects
                .iter()
                .filter_map(|project| {
                    if path.starts_with(project.key()) {
                        Some(project.get_declaration(
                            path.clone(),
                            params.text_document_position_params.position,
                        ))
                    } else {
                        None
                    }
                })
                .flatten()
                .collect(),
        )))
        // Err(Error::method_not_found())
    }
}

impl Backend {
    async fn uri_to_path(&self, uri: &Url) -> Result<PathBuf, Error> {
        match uri_to_path(uri) {
            Ok(path) => Ok(path),
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("couldn't open file with uri={:?} due to {:?}", uri, e),
                    )
                    .await;
                return Err(Error::parse_error());
            }
        }
    }
}
