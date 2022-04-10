use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use std::ffi::OsStr;

use crate::{
    project_spec::DbtProjectSpec,
    sql_file::{is_sql_file, ModelFile},
};

#[derive(Debug)]
pub struct DbtProject {
    root_path: PathBuf,
    spec: DbtProjectSpec,
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
    pub async fn from_root(project_path: &Path) -> Result<Self, String> {
        let spec = DbtProjectSpec::from_file(project_path).await?;
        let root_path = match project_path.parent() {
            None => return Err("unexpected filesystem state".to_string()),
            Some(p) => p.to_path_buf(),
        };

        Ok(Self { root_path, spec })
    }

    pub async fn refresh_spec(&mut self) -> Option<String> {
        let spec = match DbtProjectSpec::from_file(&self.root_path.join("dbt_project.yml")).await {
            Err(msg) => return Some(msg),
            Ok(spec) => spec,
        };
        self.spec = spec;
        None
    }

    fn get_sql_files_in_paths(&self, paths: &[String]) -> Vec<PathBuf> {
        paths
            .iter()
            .map(|path| {
                let sub_root = self.root_path.join(path);
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

    pub fn get_model_paths(&self) -> Vec<PathBuf> {
        self.get_sql_files_in_paths(&self.spec.model_paths)
    }

    pub fn get_macro_paths(&self) -> Vec<PathBuf> {
        self.get_sql_files_in_paths(&self.spec.macro_paths)
    }

    pub fn is_file_model(&self, path: &Path) -> bool {
        if !is_sql_file(path) {
            false
        } else {
            self.spec
                .model_paths
                .iter()
                .any(|model_root| path.starts_with(self.root_path.join(model_root)))
        }
    }

    pub fn is_file_macro(&self, path: &Path) -> bool {
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
