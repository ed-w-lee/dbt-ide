use std::intrinsics::transmute;

use rowan::{GreenNode, GreenNodeBuilder};

use super::lexer::{Token, TokenKind};

include!(concat!(env!("OUT_DIR"), "/syntax_kinds.rs"));

use SyntaxKind::*;

// Copying from https://github.com/rust-analyzer/rowan/blob/b90d7760968e0db3a6ff4bb6e919162c4023b1ff/examples/s_expressions.rs

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Lang {}

impl rowan::Language for Lang {
    type Kind = SyntaxKind;
    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        raw.0.into()
    }
    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

struct Parse {
    green_node: GreenNode,
    #[allow(unused)]
    errors: Vec<String>,
}

fn parse(tokens: Vec<Token>) -> Parse {
    struct Parser {
        tokens: Vec<Token>,
        builder: GreenNodeBuilder<'static>,
        errors: Vec<String>,
    }

    impl Parser {
        fn parse(mut self) -> Parse {
            self.builder.start_node(Template.into());
            loop {}

            todo!()
        }

        fn current(&self) -> Option<TokenKind> {
            self.tokens.last().map(|tok| tok.kind)
        }
    }

    Parser {
        tokens,
        builder: GreenNodeBuilder::new(),
        errors: Vec::new(),
    }
    .parse()
}

#[cfg(test)]
mod tests {
    // degenerate cases
    // '{% if 1 in [1,2] in [[1, 2], None] %} something {% endif %}'
}
