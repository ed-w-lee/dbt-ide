use std::{collections::HashMap, path::PathBuf};

use dashmap::DashMap;
use dbt_jinja_parser::parser::SyntaxKind;
use futures::future::{join_all, try_join_all};
use tokio::sync::RwLock;
use tower_lsp::{
    jsonrpc::Error,
    lsp_types::{
        CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams,
        CompletionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
        DidOpenTextDocumentParams, InitializeParams, InitializeResult, InsertTextFormat,
        MessageType, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
    },
    Client, LanguageServer,
};

use crate::{
    model::Macro,
    project::DbtProject,
    sql_file::{is_sql_file, MacroFile, ModelFile},
    utils::{read_file, uri_to_path},
};

type JsonRpcResult<T> = tower_lsp::jsonrpc::Result<T>;

pub struct Backend {
    pub client: Client,
    pub project: RwLock<Option<DbtProject>>,
    /// Concurrent hashmap from model file path to the in-memory
    /// parsed information for the model.
    pub models: DashMap<PathBuf, ModelFile>,
    /// Concurrent hashmap from macro file path to the in-memory
    /// parsed information for the macros.
    pub macros: DashMap<PathBuf, MacroFile>,
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

        let found_macro_paths = project.get_macro_paths();

        let parsed_macros = match try_join_all(
            found_macro_paths
                .iter()
                .map(|macro_path| MacroFile::from_file_path(macro_path)),
        )
        .await
        {
            Ok(macros) => macros,
            Err(e) => return Err(Error::parse_error()),
        };

        self.macros.clear();
        found_macro_paths
            .into_iter()
            .zip(parsed_macros.into_iter())
            .for_each(|(p, m)| {
                self.macros.insert(p, m);
            });

        {
            let mut saved_project = self.project.write().await;
            *saved_project = Some(project);
        }

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
                            "couldn't open file with uri={:?} due to {:?}",
                            params.text_document.uri, e
                        ),
                    )
                    .await;
                return;
            }
        };
        let read_project = &*self.project.read().await;
        let project = read_project.as_ref().unwrap();
        if project.is_file_model(&path) {
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
        } else if project.is_file_macro(&path) {
            let file_contents = params.text_document.text;
            let parsed_macro = match MacroFile::from_file(&path, &file_contents) {
                Ok(macro_file) => macro_file,
                Err(e) => {
                    self.client
                        .log_message(
                            MessageType::ERROR,
                            format!(
                                "couldn't parse macro file with path={:?} due to {:?}",
                                params.text_document.uri, e
                            ),
                        )
                        .await;
                    return;
                }
            };
            self.macros.insert(path, parsed_macro);
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        eprintln!("did_change");
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
        let read_project = &*self.project.read().await;
        let project = read_project.as_ref().unwrap();
        if project.is_file_model(&path) {
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
            let model_file = model_file.value_mut();
            model_file.refresh(file_contents);
        } else if project.is_file_macro(&path) {
            let file_contents = &params.content_changes[0].text;
            let mut macro_file = match self.macros.get_mut(&path) {
                None => {
                    self.client
                        .log_message(
                            MessageType::ERROR,
                            format!(
                                "couldn't find entry for macro file with path={:?}",
                                params.text_document.uri
                            ),
                        )
                        .await;
                    return;
                }
                Some(m) => m,
            };
            let macro_file = macro_file.value_mut();
            macro_file.refresh(file_contents);
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
                            "couldn't open file with uri={:?} due to {:?}",
                            params.text_document.uri, e
                        ),
                    )
                    .await;
                return;
            }
        };
        let read_project = &*self.project.read().await;
        let project = read_project.as_ref().unwrap();
        if project.is_file_model(&path) {
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
        } else if project.is_file_macro(&path) {
            if !path.exists() {
                self.macros.remove(&path);
                return;
            }
            let mut macro_file = match self.macros.get_mut(&path) {
                None => {
                    self.client
                        .log_message(
                            MessageType::ERROR,
                            format!(
                                "couldn't find entry for macro file with path={:?}",
                                params.text_document.uri
                            ),
                        )
                        .await;
                    return;
                }
                Some(m) => m,
            };
            match macro_file.value_mut().refresh_with_path(&path).await {
                Ok(_) => (),
                Err(e) => {
                    self.client
                        .log_message(
                            MessageType::ERROR,
                            format!(
                                "failed to refresh macro file with path={:?} because {:?}",
                                path, e
                            ),
                        )
                        .await;
                    return;
                }
            }
        }
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> JsonRpcResult<Option<CompletionResponse>> {
        let mut completion_items = Vec::new();

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
                return Ok(Some(CompletionResponse::Array(completion_items)));
            }
        };

        let read_project = &*self.project.read().await;
        let project = read_project.as_ref().unwrap();
        let (offset, syntax_tree) = {
            if project.is_file_model(&path) {
                match self.models.get(&path) {
                    None => {
                        self.client
                            .log_message(
                                MessageType::ERROR,
                                format!("couldn't find entry for file with path={:?}", path),
                            )
                            .await;
                        return Ok(Some(CompletionResponse::Array(completion_items)));
                    }
                    Some(model_file) => (
                        model_file
                            .position_finder
                            .get_offset(params.text_document_position.position),
                        model_file.parsed_repr.syntax(),
                    ),
                }
            } else if project.is_file_macro(&path) {
                match self.macros.get(&path) {
                    None => {
                        self.client
                            .log_message(
                                MessageType::ERROR,
                                format!("couldn't find entry for file with path={:?}", path),
                            )
                            .await;
                        return Ok(Some(CompletionResponse::Array(completion_items)));
                    }
                    Some(macro_file) => (
                        macro_file
                            .position_finder
                            .get_offset(params.text_document_position.position),
                        macro_file.parsed_repr.syntax(),
                    ),
                }
            } else {
                return Ok(Some(CompletionResponse::Array(vec![])));
            }
        };
        eprintln!(
            "position={:?} and offset={:?}",
            params.text_document_position.position, offset
        );
        let token = syntax_tree.token_at_offset(offset.into());
        eprintln!("current position: {:?}", token);
        match token {
            rowan::TokenAtOffset::None => (),
            rowan::TokenAtOffset::Single(leaf) => {}
            rowan::TokenAtOffset::Between(left, right) => {
                if left.kind() == SyntaxKind::LeftParen {
                    let call_node = left
                        .ancestors()
                        .find(|ancestor| ancestor.kind() == SyntaxKind::ExprCall);
                    match call_node {
                        None => (),
                        Some(call_node) => match call_node
                            .children()
                            .find(|child| child.kind() == SyntaxKind::ExprName)
                        {
                            None => (),
                            Some(name_node) => match name_node.last_child_or_token().unwrap() {
                                rowan::NodeOrToken::Node(_) => unreachable!(),
                                rowan::NodeOrToken::Token(token) => {
                                    if token.text() == "ref" {
                                        completion_items.extend(
                                            self.get_model_names().into_iter().map(|name| {
                                                CompletionItem {
                                                    label: format!("'{}'", name),
                                                    kind: Some(CompletionItemKind::FILE),
                                                    ..Default::default()
                                                }
                                            }),
                                        );
                                    }
                                }
                            },
                        },
                    }
                }
                if left
                    .ancestors()
                    .find(|ancestor| match ancestor.kind() {
                        SyntaxKind::Variable => true,
                        _ => false,
                    })
                    .is_some()
                {
                    eprintln!("looking for macros {:#?}", self.macros);
                    completion_items.extend(self.get_macros().into_iter().filter_map(|mac| {
                        mac.name.map(|macro_name| {
                            let mut i = 0;
                            let mut insert_text = macro_name.clone() + "(";
                            for arg in mac.args {
                                if i > 0 {
                                    insert_text.push_str(", ");
                                }
                                i = i + 1;
                                insert_text.push_str(&format!(
                                    "${{{}:{}}}",
                                    i,
                                    arg.unwrap_or("".to_string())
                                ));
                            }
                            insert_text.push(')');
                            CompletionItem {
                                label: macro_name,
                                kind: Some(CompletionItemKind::FUNCTION),
                                insert_text_format: Some(InsertTextFormat::SNIPPET),
                                insert_text: Some(insert_text),
                                ..Default::default()
                            }
                        })
                    }));
                }
            }
        }

        eprintln!("completion_items {:?}", completion_items);
        Ok(Some(CompletionResponse::Array(completion_items)))
    }
}

impl Backend {
    fn get_model_names(&self) -> Vec<String> {
        self.models
            .iter()
            .map(|model| model.value().name.clone())
            .collect()
    }

    fn get_macros(&self) -> Vec<Macro> {
        self.macros
            .iter()
            .map(|macro_file| macro_file.macros.clone())
            .flatten()
            .collect()
    }
}
