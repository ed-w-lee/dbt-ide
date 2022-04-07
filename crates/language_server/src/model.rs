use tower_lsp::lsp_types::{Location, Range};

/**
 * Inter-file metadata
 */

#[derive(Debug)]
pub struct Macro {
    name: String,
    caller_args: Vec<String>,
    args: Vec<String>,
    definition: Location,
}

#[derive(Debug)]
pub struct Materialization {
    name: String,
    definition: Location,
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
