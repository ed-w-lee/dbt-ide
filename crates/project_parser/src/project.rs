use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

use crate::spec::DbtProjectSpec;

#[derive(Debug)]
pub struct DbtProject {
    root_path: PathBuf,
    spec: DbtProjectSpec,
}

impl DbtProject {
    // TODO: better errors
    pub fn from_root(project_path: &Path) -> Result<Self, String> {
        let spec = DbtProjectSpec::from_file(project_path)?;
        let root_path = match project_path.parent() {
            None => return Err("unexpected filesystem state".to_string()),
            Some(p) => p.to_path_buf(),
        };

        Ok(Self { root_path, spec })
    }

    pub fn refresh_spec(&mut self) -> Option<String> {
        let spec = match DbtProjectSpec::from_file(&self.root_path.join("dbt_project.yml")) {
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
                            if e.path().extension() == Some(OsStr::new("sql")) {
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
