use std::path::Path;

use dbt_jinja_parser::lexer::tokenize;
use dbt_jinja_parser::parser::{parse, Parse};
use derivative::Derivative;

use crate::position_finder::PositionFinder;
use crate::utils::read_file;

#[derive(Derivative)]
#[derivative(Debug)]
/// This represents the metadata we need to track for a dbt model file.
pub struct ModelFile {
    pub name: String,
    pub position_finder: PositionFinder,
    #[derivative(Debug = "ignore")]
    pub parsed_repr: Parse,
}

impl ModelFile {
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
}
