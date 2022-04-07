use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Included};

use tower_lsp::lsp_types::Position;

#[derive(Debug)]
struct PositionFinder {
    /// List of the positions of newlines
    newline_index: BTreeMap<u32, u32>,
}

impl PositionFinder {
    fn from_text(text: &str) -> Self {
        let mut newline_index = BTreeMap::new();
        text.char_indices().fold(0, |acc, (pos, char)| match char {
            '\n' | '\r' => {
                newline_index.insert(pos as u32, acc);
                acc + 1
            }
            _ => acc,
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
