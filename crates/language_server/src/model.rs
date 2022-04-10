use std::path::PathBuf;

use tower_lsp::lsp_types::{Location, Range};

/**
 * Inter-file metadata
 */

#[derive(Debug, Clone)]
pub struct Macro {
    pub name: Option<String>,
    pub args: Vec<Option<String>>,
    pub default_args: Vec<(Option<String>, Option<String>)>,
}

#[derive(Debug)]
pub struct Materialization {
    pub name: String,
    pub definition: Location,
}

#[derive(Debug)]
pub struct Source {
    name: String,
    definition: Location,
}

#[derive(Debug)]
pub struct GenericTest {
    name: String,
    args: Vec<String>,
    definition: Location,
}

/**
 * Intra-file metadata
 */

#[derive(Debug)]
pub struct Object {
    name: String,
    declaration: Range,
    scope: Range,
}
