use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Included};

use lspower::lsp::Position;

use crate::model::{Macro, Materialization, ModelRef, Object};

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

/// This represents the metadata we need to track if a dbt jinja file is open
struct SqlFileFull {
    file_reduced: SqlFileReduced,
    position_finder: PositionFinder,
    objects: Vec<Object>,
}

/// This represents the metadata we need to track regardless of if a dbt jinja
/// file is open or not
struct SqlFileReduced {
    macros: Vec<Macro>,
    materializations: Vec<Materialization>,
    model: Option<ModelRef>,
}
