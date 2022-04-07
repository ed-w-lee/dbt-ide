use std::{collections::HashMap, path::PathBuf};

use dashmap::DashMap;
use futures::future::{join_all, try_join_all};
use tokio::sync::RwLock;
use tower_lsp::{
    jsonrpc::Error,
    lsp_types::{
        CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams,
        CompletionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
        DidOpenTextDocumentParams, InitializeParams, InitializeResult, MessageType,
        ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
    },
    Client, LanguageServer,
};

use crate::{
    project::DbtProject,
    sql_file::ModelFile,
    utils::{read_file, uri_to_path},
};

type JsonRpcResult<T> = tower_lsp::jsonrpc::Result<T>;

pub struct Backend {
    pub client: Client,
    /// Concurrent hashmap from stringified model path to the in-memory
    /// parsed information for the model.
    pub models: DashMap<PathBuf, ModelFile>,
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

        let found_model_paths = project.get_model_paths();

        let parsed_models = match try_join_all(
            found_model_paths
                .iter()
                .map(|model_path| ModelFile::from_file_path(model_path)),
        )
        .await
        {
            Ok(models) => models,
            Err(e) => return Err(Error::parse_error()),
        };

        self.models.clear();
        found_model_paths
            .into_iter()
            .zip(parsed_models.into_iter())
            .for_each(|(p, m)| {
                self.models.insert(p, m);
            });

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
                            "couldn't open file with uri={:?} due to {:?}",
                            params.text_document.uri, e
                        ),
                    )
                    .await;
                return;
            }
        };
        if !ModelFile::is_sql_file(&path) {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "not parsing file with path={:?} because it is not a sql file",
                        params.text_document.uri
                    ),
                )
                .await;
            return;
        }

        let file_contents = params.text_document.text;
        let parsed_model = match ModelFile::from_file(&path, &file_contents) {
            Ok(model) => model,
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!(
                            "couldn't parse model file with path={:?} due to {:?}",
                            params.text_document.uri, e
                        ),
                    )
                    .await;
                return;
            }
        };
        self.models.insert(path, parsed_model);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let path = match uri_to_path(&params.text_document.uri) {
            Ok(path) => path,
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!(
                            "couldn't open file with uri={:?} due to {:?}",
                            params.text_document.uri, e
                        ),
                    )
                    .await;
                return;
            }
        };
        if !ModelFile::is_sql_file(&path) {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "not parsing file with path={:?} because it is not a sql file",
                        params.text_document.uri
                    ),
                )
                .await;
            return;
        }

        let file_contents = &params.content_changes[0].text;
        let mut model_file = match self.models.get_mut(&path) {
            None => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!(
                            "couldn't find entry for model file with path={:?}",
                            params.text_document.uri
                        ),
                    )
                    .await;
                return;
            }
            Some(m) => m,
        };
        model_file.value_mut().refresh(file_contents);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let path = match uri_to_path(&params.text_document.uri) {
            Ok(path) => path,
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!(
                            "couldn't open file with uri={:?} due to {:?}",
                            params.text_document.uri, e
                        ),
                    )
                    .await;
                return;
            }
        };
        if !ModelFile::is_sql_file(&path) {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "not parsing file with path={:?} because it is not a sql file",
                        params.text_document.uri
                    ),
                )
                .await;
        }

        if !path.exists() {
            self.models.remove(&path);
            return;
        }
        let mut model_file = match self.models.get_mut(&path) {
            None => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!(
                            "couldn't find entry for model file with path={:?}",
                            params.text_document.uri
                        ),
                    )
                    .await;
                return;
            }
            Some(m) => m,
        };
        match model_file.value_mut().refresh_with_path(&path).await {
            Ok(_) => (),
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!(
                            "failed to refresh model file with path={:?} because {:?}",
                            path, e
                        ),
                    )
                    .await;
                return;
            }
        }
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> JsonRpcResult<Option<CompletionResponse>> {
        let current_uri = params.text_document_position.text_document.uri;

        let current_pos = params.text_document_position.position;
        let file_contents = match read_file(&uri_to_path(&current_uri)?).await {
            Ok(contents) => contents,
            Err(e) => return Err(Error::internal_error()),
        };

        // Ok(Some(CompletionResponse::Array(
        //     self.models
        //         .iter()
        //         .map(|model| CompletionItem {
        //             label: model.to_owned(),
        //             kind: Some(CompletionItemKind::VARIABLE),
        //             ..Default::default()
        //         })
        //         .collect(),
        // )))
        todo!()
    }
}
