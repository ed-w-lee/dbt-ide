use dashmap::DashMap;
use dbt_jinja_parser::parser::SyntaxKind;
use derivative::Derivative;
use futures::future::{self, try_join_all};
use futures::{TryFuture, TryFutureExt};
use std::fs::FileType;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, InsertTextFormat, LocationLink, Position,
};
use walkdir::WalkDir;

use crate::entity::{Macro, BUILTIN_MACROS};
use crate::files::macro_file::MacroFile;
use crate::files::model_file::ModelFile;
use crate::files::project_yml::DbtProjectSpec;
use crate::utils::{get_child_of_kind, is_sql_file, SyntaxNode, TraverseOrder};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct DbtProject {
    root_path: PathBuf,
    spec: DbtProjectSpec,
    /// Concurrent hashmap from model file path to the in-memory
    /// parsed information for the model.
    pub models: DashMap<PathBuf, ModelFile>,
    /// Concurrent hashmap from macro file path to the in-memory
    /// parsed information for the macros.
    pub macros: DashMap<PathBuf, MacroFile>,
    /// Installed packages
    pub packages: DashMap<PathBuf, DbtProject>,
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
    /// searches for a single project at the root path (since dbt sucks at
    /// disambiguating multiple projects)
    pub async fn find_single_project(root_path: &Path) -> Result<Self, String> {
        let entry = root_path.join("dbt_project.yml");
        if entry.exists() {
            match DbtProject::from_root(entry.as_path()).await {
                Ok(project) => Ok(project),
                Err(msg) => Err(msg),
            }
        } else {
            Err("couldn't find dbt_project.yml".to_string())
        }
    }

    async fn parse_package(project_path: &Path) -> Result<Self, String> {
        let spec = DbtProjectSpec::from_file_path(project_path).await?;
        let root_path = match project_path.parent() {
            None => return Err("unexpected filesystem state".to_string()),
            Some(p) => p.to_path_buf(),
        };

        tracing::debug!("parsing models");
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

        tracing::debug!("parsing macros");
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
            packages: DashMap::new(),
        })
    }

    // TODO: better errors
    async fn from_root(project_path: &Path) -> Result<Self, String> {
        let mut project = Self::parse_package(project_path).await?;

        project.packages = {
            let mut packages = vec![];
            let install_path = Path::new(&project.spec.packages_install_path);
            let entries = match install_path.read_dir() {
                Ok(entries) => entries,
                Err(e) => return Err(format!("failed to get installed packages: {}", e)),
            };
            for entry in entries {
                match entry {
                    Ok(entry) => match entry.file_type() {
                        Ok(file_type) => {
                            if file_type.is_dir() {
                                let possible_package = entry.path().join("dbt_project.yml");
                                tracing::debug!(?possible_package);
                                if possible_package.exists() {
                                    match DbtProject::parse_package(&possible_package).await {
                                        Ok(package) => {
                                            packages.push((entry.path().to_owned(), package))
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                message = "failed to parse package",
                                                path = ?possible_package,
                                                error = ?e
                                            );
                                        }
                                    }
                                } else {
                                    tracing::warn!(
                                        message = "couldn't find dbt_project.yml",
                                        ?entry
                                    );
                                }
                            } else {
                                tracing::debug!(
                                    message = "found non-directory in packages install",
                                    ?entry,
                                    ?file_type
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!(message = "unable to read file type for entry", ?entry, error = ?e);
                        }
                    },
                    Err(ref e) => {
                        tracing::warn!(message = "failed to get entry after readdir", ?entry, error = ?e);
                    }
                }
            }
            packages.into_iter().collect()
        };

        Ok(project)
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
                                label: name.clone(),
                                insert_text: Some(format!("'{}'", &name)),
                                kind: Some(CompletionItemKind::FILE),
                                detail: Some("Model".to_string()),
                                sort_text: Some(format!("'{}'", &name)),
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
        let mut to_return: Vec<CompletionItem> = self
            .get_macros()
            .into_iter()
            .filter_map(|mac| mac.get_completion_items(None))
            .collect();

        to_return.extend(BUILTIN_MACROS.iter().map(|m| m.get_completion_items()));

        to_return.extend(self.packages.iter().flat_map(|project| {
            let project_name = &project.spec.name;
            project
                .get_macros()
                .into_iter()
                .filter_map(|mac| mac.get_completion_items(Some(&project_name)))
                .collect::<Vec<_>>()
        }));

        to_return
    }

    pub fn get_completion_items(&self, path: PathBuf, position: Position) -> Vec<CompletionItem> {
        let mut completion_items = Vec::new();

        let (offset, syntax_tree) = {
            if self.is_file_model(&path) {
                match self.models.get(&path) {
                    None => {
                        tracing::error!(
                            message = "couldn't find model corresponding to path",
                            path = ?path
                        );
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
                        tracing::error!(message = "couldn't find macro corresponding to path", path = ?path);
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
        tracing::debug!(
            message = "map from position to offset",
            position = ?position,
            offset = ?offset
        );
        let token = syntax_tree.token_at_offset(offset.into());
        tracing::debug!(message = "current token", token = ?token);
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
                    tracing::debug!(message = "looking for macros", macros = ?self.macros);
                    completion_items.extend(self.get_macro_completion());
                }
            }
        }

        completion_items
    }

    fn get_model_declaration(&self, call_node: &SyntaxNode) -> Vec<LocationLink> {
        match get_child_of_kind(call_node, SyntaxKind::ExprName, TraverseOrder::Forward) {
            None => vec![],
            Some(name_node) => match name_node.last_child_or_token().unwrap() {
                rowan::NodeOrToken::Node(_) => unreachable!(),
                rowan::NodeOrToken::Token(token) => {
                    if token.text() == "ref" {
                        match call_node
                            .descendants()
                            .find(|node| node.kind() == SyntaxKind::CallStaticArg)
                        {
                            None => vec![],
                            Some(arg_node) => {
                                match get_child_of_kind(
                                    &arg_node,
                                    SyntaxKind::ExprConstantString,
                                    TraverseOrder::Forward,
                                ) {
                                    None => vec![],
                                    Some(model_name_node) => {
                                        let model_name = model_name_node.text().to_string();
                                        let model_path =
                                            self.get_model_path_from_string(model_name.trim());
                                        match model_path {
                                            None => vec![],
                                            Some(path) => {
                                                vec![LocationLink {
                                                    origin_selection_range: todo!(),
                                                    target_uri: todo!(),
                                                    target_range: todo!(),
                                                    target_selection_range: todo!(),
                                                }]
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        vec![]
                    }
                }
            },
        }
    }

    pub fn get_declaration(&self, path: PathBuf, position: Position) -> Vec<LocationLink> {
        let mut locations = Vec::new();

        let (offset, syntax_tree) = {
            if self.is_file_model(&path) {
                match self.models.get(&path) {
                    None => {
                        tracing::error!(
                            message = "couldn't find model corresponding to path",
                            path = ?path
                        );
                        return locations;
                    }
                    Some(model_file) => (
                        model_file.position_finder.get_offset(position),
                        model_file.parsed_repr.syntax(),
                    ),
                }
            } else if self.is_file_macro(&path) {
                match self.macros.get(&path) {
                    None => {
                        tracing::error!(message= "couldn't find macro corresponding to path", path=?path);
                        return locations;
                    }
                    Some(macro_file) => (
                        macro_file.position_finder.get_offset(position),
                        macro_file.parsed_repr.syntax(),
                    ),
                }
            } else {
                return locations;
            }
        };
        tracing::debug!(message = "position to offset", ?position, ?offset);
        let token = syntax_tree.token_at_offset(offset.into());
        tracing::debug!(message = "token at offset", ?token);
        match token {
            rowan::TokenAtOffset::None => (),
            rowan::TokenAtOffset::Single(leaf) => {
                let call_node = leaf
                    .ancestors()
                    .find(|ancestor| ancestor.kind() == SyntaxKind::ExprCall);
                match call_node {
                    None => (),
                    Some(node) => locations.extend(self.get_model_declaration(&node)),
                }
            }
            rowan::TokenAtOffset::Between(left, right) => (),
        }

        locations
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

    fn get_model_path_from_string(&self, literal_str: &str) -> Option<PathBuf> {
        self.models.iter().find_map(|model| {
            let name = &model.value().name;
            if &format!("'{literal_str}'") == name || &format!("\"{literal_str}\"") == name {
                Some(model.key().clone())
            } else {
                None
            }
        })
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
