use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::utils::read_file;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct DbtProjectSpec {
    #[serde(rename = "model-paths", default = "default_model_paths")]
    pub model_paths: Vec<String>,
    #[serde(rename = "macro-paths", default = "default_macro_paths")]
    pub macro_paths: Vec<String>,
}

fn default_model_paths() -> Vec<String> {
    vec!["models".to_string()]
}

fn default_macro_paths() -> Vec<String> {
    vec!["macros".to_string()]
}

impl DbtProjectSpec {
    // TODO: add better errors
    pub async fn from_file(path: &Path) -> Result<Self, String> {
        let project = serde_yaml::from_str::<DbtProjectSpec>(&read_file(path).await?);
        let project = match project {
            Err(e) => return Err(format!("bad yaml parse: {:?}", e)),
            Ok(project) => project,
        };

        Ok(project)
    }
}
