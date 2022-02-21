use lspower::lsp::{Location, Range};

/**
 * Inter-file metadata
 */

pub struct Macro {
    name: String,
    caller_args: Vec<String>,
    args: Vec<String>,
    definition: Location,
}

pub struct Materialization {
    name: String,
    definition: Location,
}

pub struct ModelRef {
    name: String,
    definition: Location,
}

pub struct Source {
    name: String,
    definition: Location,
}

pub struct GenericTest {
    name: String,
    args: Vec<String>,
    definition: Location,
}

/**
 * Intra-file metadata
 */

pub struct Object {
    name: String,
    declaration: Range,
    scope: Range,
}
