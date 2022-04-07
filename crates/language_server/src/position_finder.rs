use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Included};

use tower_lsp::lsp_types::Position;

#[derive(Debug)]
pub struct PositionFinder {
    /// offset -> newline
    offset_to_newline: BTreeMap<u32, u32>,
    /// newline -> offset
    newline_to_offset: Vec<u32>,
}

impl PositionFinder {
    pub fn from_text(text: &str) -> Self {
        let mut offset_to_newline = BTreeMap::new();
        let mut newline_to_offset = Vec::new();
        text.char_indices().fold(0, |acc, (pos, char)| match char {
            '\n' | '\r' => {
                offset_to_newline.insert(pos as u32, acc);
                newline_to_offset.push(pos as u32);
                acc + 1
            }
            _ => acc,
        });
        Self {
            offset_to_newline,
            newline_to_offset,
        }
    }

    pub fn get_lineno(&self, idx: u32) -> u32 {
        match self
            .offset_to_newline
            .range((Included(&0), Excluded(&idx)))
            .last()
        {
            Some((_, lineno)) => *lineno,
            None => 0,
        }
    }

    pub fn get_position(&self, idx: u32) -> Position {
        match self
            .offset_to_newline
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

    pub fn get_offset(&self, position: Position) -> u32 {
        match self.newline_to_offset.get(position.line as usize) {
            Some(line_offset) => line_offset + position.character,
            None => todo!(),
        }
    }
}
