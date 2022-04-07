use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::ops::Bound::{Excluded, Included};
use std::path::{Path, PathBuf};

use dbt_jinja_parser::lexer::tokenize;
use dbt_jinja_parser::parser::{parse, Parse};
use tokio::fs::read;
use tower_lsp::lsp_types::{Location, Position};

use crate::model::{Macro, Materialization, Object};
use crate::position_finder::PositionFinder;
use crate::utils::read_file;

/// This represents the metadata we need to track for a dbt model file.
pub struct ModelFile {
    name: String,
    position_finder: PositionFinder,
    parsed_repr: Parse,
}

impl ModelFile {
    pub fn is_sql_file(path: &Path) -> bool {
        path.extension() == Some(OsStr::new("sql"))
    }

    pub fn from_file(file_path: &Path, file_contents: &str) -> Result<Self, String> {
        let name = match file_path.file_stem() {
            None => return Err(format!("no file stem found for {:?}", file_path)),
            Some(stem) => stem.to_string_lossy(),
        };
        Ok(Self {
            name: name.to_string(),
            position_finder: PositionFinder::from_text(file_contents),
            parsed_repr: parse(tokenize(file_contents)),
        })
    }

    pub async fn from_file_path(file_path: &Path) -> Result<Self, String> {
        let file_contents = read_file(file_path).await?;
        Self::from_file(file_path, &file_contents)
    }

    pub fn refresh(&mut self, file_contents: &str) {
        self.position_finder = PositionFinder::from_text(file_contents);
        self.parsed_repr = parse(tokenize(file_contents));
    }

    pub async fn refresh_with_path(&mut self, file_path: &Path) -> Result<(), String> {
        let file_contents = read_file(file_path).await?;
        self.refresh(&file_contents);
        Ok(())
    }
}
