use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::utils::read_file;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct DbtProjectSpec {
    pub name: String,
    #[serde(rename = "model-paths", default = "default_model_paths")]
    pub model_paths: Vec<String>,
    #[serde(rename = "macro-paths", default = "default_macro_paths")]
    pub macro_paths: Vec<String>,
    #[serde(
        rename = "packages-install-path",
        default = "default_packages_install_path"
    )]
    pub packages_install_path: String,
}

fn default_model_paths() -> Vec<String> {
    vec!["models".to_string()]
}

fn default_macro_paths() -> Vec<String> {
    vec!["macros".to_string()]
}

fn default_packages_install_path() -> String {
    "dbt_packages".to_string()
}

impl DbtProjectSpec {
    // TODO: add better errors
    pub fn from_file(file_contents: &str) -> Result<Self, String> {
        let project = serde_yaml::from_str::<DbtProjectSpec>(file_contents);
        let project = match project {
            Err(e) => return Err(format!("bad yaml parse: {:?}", e)),
            Ok(project) => project,
        };

        Ok(project)
    }

    pub async fn from_file_path(file_path: &Path) -> Result<Self, String> {
        let file_contents = read_file(file_path).await?;
        Self::from_file(&file_contents)
    }

    pub fn refresh(&mut self, file_contents: &str) -> Result<(), String> {
        *self = Self::from_file(file_contents)?;
        Ok(())
    }
}
