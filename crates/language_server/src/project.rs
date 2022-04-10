use dashmap::DashMap;
use dbt_jinja_parser::parser::SyntaxKind;
use futures::future::try_join_all;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat, Position};
use walkdir::WalkDir;

use crate::files::project_yml::DbtProjectSpec;
use crate::model::Macro;
use crate::sql_file::{is_sql_file, MacroFile, ModelFile};
use crate::utils::SyntaxNode;

#[derive(Debug)]
pub struct DbtProject {
    root_path: PathBuf,
    spec: DbtProjectSpec,
    /// Concurrent hashmap from model file path to the in-memory
    /// parsed information for the model.
    pub models: DashMap<PathBuf, ModelFile>,
    /// Concurrent hashmap from macro file path to the in-memory
    /// parsed information for the macros.
    pub macros: DashMap<PathBuf, MacroFile>,
}

fn get_sql_files_in_paths(root_path: &Path, paths: &[String]) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|path| {
            let sub_root = root_path.join(path);
            WalkDir::new(sub_root).into_iter().filter_map(|e| match e {
                Err(_) => None,
                Ok(e) => {
                    if is_sql_file(e.path()) {
                        Some(e.path().to_path_buf())
                    } else {
                        None
                    }
                }
            })
        })
        .flatten()
        .collect()
}

impl DbtProject {
    pub async fn find_single_project(root_path: &Path) -> Result<Self, String> {
        let mut err_msg = "couldn't find dbt_project.yml".to_string();
        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let f_name = entry.file_name().to_string_lossy();
            if f_name == "dbt_project.yml" {
                match DbtProject::from_root(entry.path()).await {
                    Ok(project) => return Ok(project),
                    Err(msg) => err_msg = msg,
                }
            }
        }
        Err(err_msg)
    }

    // TODO: better errors
    async fn from_root(project_path: &Path) -> Result<Self, String> {
        let spec = DbtProjectSpec::from_file_path(project_path).await?;
        let root_path = match project_path.parent() {
            None => return Err("unexpected filesystem state".to_string()),
            Some(p) => p.to_path_buf(),
        };

        let models = {
            let found_model_paths = get_sql_files_in_paths(&root_path, &spec.model_paths);

            let parsed_models = match try_join_all(
                found_model_paths
                    .iter()
                    .map(|model_path| ModelFile::from_file_path(model_path)),
            )
            .await
            {
                Ok(models) => models,
                Err(e) => return Err(format!("failed to parse models: {}", e)),
            };

            found_model_paths
                .into_iter()
                .zip(parsed_models.into_iter())
                .collect()
        };

        let macros = {
            let found_macro_paths = get_sql_files_in_paths(&root_path, &spec.macro_paths);

            let parsed_macros = match try_join_all(
                found_macro_paths
                    .iter()
                    .map(|macro_path| MacroFile::from_file_path(macro_path)),
            )
            .await
            {
                Ok(macros) => macros,
                Err(e) => return Err(format!("failed to parse macros: {}", e)),
            };

            found_macro_paths
                .into_iter()
                .zip(parsed_macros.into_iter())
                .collect()
        };

        Ok(Self {
            root_path,
            spec,
            models,
            macros,
        })
    }

    pub fn on_file_open(&self, path: &Path, file_contents: &str) -> Result<(), String> {
        if self.is_file_model(&path) {
            match ModelFile::from_file(&path, &file_contents) {
                Ok(model) => {
                    self.models.insert(path.to_path_buf(), model);
                    Ok(())
                }
                Err(e) => Err(format!(
                    "couldn't parse model file with path={:?} due to {:?}",
                    path, e
                )),
            }
        } else if self.is_file_macro(&path) {
            match MacroFile::from_file(&file_contents) {
                Ok(macro_file) => {
                    self.macros.insert(path.to_path_buf(), macro_file);
                    Ok(())
                }
                Err(e) => Err(format!(
                    "couldn't parse macro file with path={:?} due to {:?}",
                    path, e
                )),
            }
        } else {
            Ok(())
        }
    }

    pub fn on_file_change(&self, path: &Path, file_contents: &str) -> Result<(), String> {
        if self.is_file_model(&path) {
            match self.models.get_mut(&path.to_path_buf()) {
                None => Err(format!(
                    "couldn't find entry for model file with path={:?}",
                    path
                )),
                Some(mut m) => {
                    m.refresh(file_contents);
                    Ok(())
                }
            }
        } else if self.is_file_macro(&path) {
            match self.macros.get_mut(&path.to_path_buf()) {
                None => Err(format!(
                    "couldn't find entry for macro file with path={:?}",
                    path
                )),
                Some(mut m) => {
                    m.refresh(file_contents);
                    Ok(())
                }
            }
        } else {
            Ok(())
        }
    }

    pub fn on_file_close(
        &self,
        path: PathBuf,
        file_contents: &Option<String>,
    ) -> Result<(), String> {
        if self.is_file_model(&path) {
            match file_contents {
                None => {
                    self.models.remove(&path);
                    Ok(())
                }
                Some(contents) => match self.models.get_mut(&path) {
                    None => Err(format!(
                        "couldn't find entry for model file with path={:?}",
                        path
                    )),
                    Some(mut m) => {
                        m.refresh(contents);
                        Ok(())
                    }
                },
            }
        } else if self.is_file_macro(&path) {
            match file_contents {
                None => {
                    self.macros.remove(&path);
                    return Ok(());
                }
                Some(contents) => match self.macros.get_mut(&path) {
                    None => Err(format!(
                        "couldn't find entry for macro file with path={:?}",
                        path
                    )),
                    Some(mut m) => {
                        m.refresh(contents);
                        Ok(())
                    }
                },
            }
        } else {
            Ok(())
        }
    }

    fn get_function_completion(&self, call_node: &SyntaxNode) -> Vec<CompletionItem> {
        match call_node
            .children()
            .find(|child| child.kind() == SyntaxKind::ExprName)
        {
            None => vec![],
            Some(name_node) => match name_node.last_child_or_token().unwrap() {
                rowan::NodeOrToken::Node(_) => unreachable!(),
                rowan::NodeOrToken::Token(token) => {
                    if token.text() == "ref" {
                        self.get_model_names()
                            .into_iter()
                            .map(|name| CompletionItem {
                                label: format!("'{}'", name),
                                kind: Some(CompletionItemKind::FILE),
                                ..Default::default()
                            })
                            .collect()
                    } else {
                        vec![]
                    }
                }
            },
        }
    }

    fn get_macro_completion(&self) -> Vec<CompletionItem> {
        self.get_macros()
            .into_iter()
            .filter_map(|mac| {
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
            })
            .collect()
    }

    pub fn get_completion_items(&self, path: PathBuf, position: Position) -> Vec<CompletionItem> {
        let mut completion_items = Vec::new();

        let (offset, syntax_tree) = {
            if self.is_file_model(&path) {
                match self.models.get(&path) {
                    None => {
                        eprintln!("couldn't find model corresponding to path={:?}", path);
                        return completion_items;
                    }
                    Some(model_file) => (
                        model_file.position_finder.get_offset(position),
                        model_file.parsed_repr.syntax(),
                    ),
                }
            } else if self.is_file_macro(&path) {
                match self.macros.get(&path) {
                    None => {
                        eprintln!("couldn't find macro corresponding to path={:?}", path);
                        return completion_items;
                    }
                    Some(macro_file) => (
                        macro_file.position_finder.get_offset(position),
                        macro_file.parsed_repr.syntax(),
                    ),
                }
            } else {
                return completion_items;
            }
        };
        eprintln!("position={:?} <==> offset={:?}", position, offset);
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
                        Some(node) => completion_items.extend(self.get_function_completion(&node)),
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
                    completion_items.extend(self.get_macro_completion());
                }
            }
        }

        completion_items
    }

    fn get_model_paths(&self) -> Vec<PathBuf> {
        get_sql_files_in_paths(&self.root_path, &self.spec.model_paths)
    }

    fn get_macro_paths(&self) -> Vec<PathBuf> {
        get_sql_files_in_paths(&self.root_path, &self.spec.macro_paths)
    }

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

    fn is_file_model(&self, path: &Path) -> bool {
        if !is_sql_file(path) {
            false
        } else {
            self.spec
                .model_paths
                .iter()
                .any(|model_root| path.starts_with(self.root_path.join(model_root)))
        }
    }

    fn is_file_macro(&self, path: &Path) -> bool {
        if !is_sql_file(path) {
            false
        } else {
            self.spec
                .macro_paths
                .iter()
                .any(|macro_root| path.starts_with(self.root_path.join(macro_root)))
        }
    }
}
