use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use std::ffi::OsStr;

use crate::{project_spec::DbtProjectSpec, sql_file::ModelFile};

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

    pub fn get_model_paths(&self) -> Vec<PathBuf> {
        let model_root_paths = &self.spec.model_paths;
        model_root_paths
            .iter()
            .map(|model_root_path| {
                let model_root = self.root_path.join(model_root_path);
                WalkDir::new(model_root)
                    .into_iter()
                    .filter_map(|e| match e {
                        Err(_) => None,
                        Ok(e) => {
                            if ModelFile::is_sql_file(e.path()) {
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
}
