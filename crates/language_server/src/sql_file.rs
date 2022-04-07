use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::ops::Bound::{Excluded, Included};
use std::path::{Path, PathBuf};

use dbt_jinja_parser::lexer::tokenize;
use dbt_jinja_parser::parser::{parse, Parse};
use tokio::fs::read;
use tower_lsp::lsp_types::{Location, Position};

use crate::model::{Macro, Materialization, Object};

#[derive(Debug)]
struct PositionFinder {
    /// List of the positions of newlines
    newline_index: BTreeMap<u32, u32>,
}

impl PositionFinder {
    fn from_text(text: &str) -> Self {
        let mut newline_index = BTreeMap::new();
        text.char_indices().fold(0, |acc, (pos, char)| {
            if char == '\n' {
                newline_index.insert(pos as u32, acc);
                acc + 1
            } else {
                acc
            }
        });
        Self { newline_index }
    }

    fn get_lineno(&self, idx: u32) -> u32 {
        match self
            .newline_index
            .range((Included(&0), Excluded(&idx)))
            .last()
        {
            Some((_, lineno)) => *lineno,
            None => 0,
        }
    }

    fn get_position(&self, idx: u32) -> Position {
        match self
            .newline_index
            .range((Included(&0), Excluded(&idx)))
            .last()
        {
            Some((pos, lineno)) => Position {
                line: *lineno,
                character: *pos - idx,
            },
            None => Position {
                line: 0,
                character: idx,
            },
        }
    }
}

/// This represents the metadata we need to track if a dbt model file is open.
pub struct ModelFileFull {
    file_reduced: ModelFileReduced,
    position_finder: PositionFinder,
    parsed_repr: Parse,
}

/// This represents the metadata we need to track regardless of if a dbt model
/// file is open or not.
pub struct ModelFileReduced {
    name: String,
}

pub enum ModelFile {
    Full(ModelFileFull),
    Reduced(ModelFileReduced),
}

impl ModelFile {
    pub fn is_sql_file(path: &Path) -> bool {
        path.extension() == Some(OsStr::new("sql"))
    }
}

impl ModelFileFull {
    pub fn from_file(file_path: &Path, file_contents: &str) -> Result<Self, String> {
        let reduced = ModelFileReduced::from_file(file_path)?;
        Ok(Self {
            file_reduced: reduced,
            position_finder: PositionFinder::from_text(file_contents),
            parsed_repr: parse(tokenize(file_contents)),
        })
    }

    pub fn to_reduced(self) -> ModelFileReduced {
        self.file_reduced
    }

    pub fn as_reduced<'a>(&'a self) -> &'a ModelFileReduced {
        &self.file_reduced
    }

    pub fn refresh(&mut self, file_contents: &str) {
        self.position_finder = PositionFinder::from_text(file_contents);
        self.parsed_repr = parse(tokenize(file_contents));
    }
}

impl ModelFileReduced {
    pub fn from_file(file_path: &Path) -> Result<Self, String> {
        let name = match file_path.file_stem() {
            None => return Err(format!("no file stem found for {:?}", file_path)),
            Some(stem) => stem.to_string_lossy(),
        };
        Ok(Self {
            name: name.to_string(),
        })
    }
}
