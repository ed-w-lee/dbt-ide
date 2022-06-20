use std::path::PathBuf;

use rowan::TextRange;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat, Location, Range};

/**
 * Inter-file metadata
 */
#[derive(Debug)]
pub struct Materialization {
    pub name: Option<String>,
    pub adapter: String,
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
