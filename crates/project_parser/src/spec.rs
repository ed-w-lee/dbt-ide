use std::{fs::read, path::Path};

use serde::{Deserialize, Serialize};

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
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let raw_bytes = read(path);
        let raw_bytes = {
            if raw_bytes.is_err() {
                return Err("bad read".to_string());
            } else {
                raw_bytes.unwrap()
            }
        };
        let project = serde_yaml::from_str::<DbtProjectSpec>(&String::from_utf8_lossy(&raw_bytes));
        let project = match project {
            Err(e) => return Err(format!("bad yaml parse: {:?}", e)),
            Ok(project) => project,
        };

        Ok(project)
    }
}
