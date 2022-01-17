use super::lexer::{Token, TokenKind};
use rowan::{GreenNode, GreenNodeBuilder};
use SyntaxKind::*;

include!(concat!(env!("OUT_DIR"), "/syntax_kinds.rs"));

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

struct Parser {
    tokens: Vec<Token>,
    builder: GreenNodeBuilder<'static>,
    errors: Vec<String>,
}

impl Parser {
    // Utilities for traversing through token stream

    fn current(&self) -> Option<TokenKind> {
        self.tokens.last().map(|tok| tok.kind)
    }

    fn bump(&mut self) {
        let token = self.tokens.pop().unwrap();
        self.builder
            .token(SyntaxKind::from(token.kind).into(), &token.text);
    }

    fn skip_ws(&mut self) {
        while self.current() == Some(TokenKind::Whitespace) {
            self.bump()
        }
    }

    fn bump_error(&mut self) {
        let token = self.tokens.pop().unwrap();
        self.builder.token(SyntaxKind::Error.into(), &token.text);
    }

    fn error_until(&mut self, token: TokenKind) {
        loop {
            match self.current() {
                None => self.errors.push(format!("never found {:?}", token)),
                Some(t) if t == token => self.bump(),
                Some(_) => self.bump_error(),
            }
        }
    }

    // Recursive descent

    fn parse(mut self) -> Parse {
        self.builder.start_node(Template.into());
        loop {
            match self.current() {
                None => break,
                Some(TokenKind::Data) => {
                    self.builder.start_node(ExprData.into());
                    self.bump();
                    self.builder.finish_node();
                }
                Some(TokenKind::RawBegin) => {
                    self.builder.start_node(StmtRaw.into());
                    self.bump();
                    self.parse_raw();
                    self.builder.finish_node();
                }
                Some(TokenKind::CommentBegin) => {
                    self.builder.start_node(Comment.into());
                    self.bump();
                    self.parse_comment();
                    self.builder.finish_node();
                }
                Some(TokenKind::VariableBegin) => {
                    self.builder.start_node(Variable.into());
                    self.bump();
                    self.parse_variable();
                    self.builder.finish_node();
                }
                Some(TokenKind::BlockBegin) => {}
                Some(_) => unreachable!(),
            }
        }

        todo!()
    }

    fn parse_tuple(&mut self, with_condexpr: bool) {
        todo!()
    }

    fn parse_variable(&mut self) {
        self.parse_tuple(true);
        self.error_until(TokenKind::VariableEnd);
    }

    fn parse_comment(&mut self) {
        match self.current() {
            None => self.errors.push("incomplete comment".into()),
            Some(TokenKind::CommentData) => self.bump(),
            Some(TokenKind::CommentEnd) => {
                self.bump();
                return;
            }
            Some(_) => unreachable!(),
        }
        match self.current() {
            None => self
                .errors
                .push("incomplete comment, expected \"#}\"".into()),
            Some(TokenKind::CommentEnd) => self.bump(),
            Some(_) => unreachable!(),
        }
    }

    fn parse_raw(&mut self) {
        match self.current() {
            None => self.errors.push("incomplete raw block".into()),
            Some(TokenKind::Data) => self.bump(),
            Some(TokenKind::RawEnd) => {
                self.bump();
                return;
            }
            Some(_) => unreachable!(),
        }
        match self.current() {
            None => self
                .errors
                .push(r#"incomplete raw block, expected "{% endraw %}""#.into()),
            Some(TokenKind::RawEnd) => self.bump(),
            Some(_) => unreachable!(),
        }
    }
}

struct Parse {
    green_node: GreenNode,
    #[allow(unused)]
    errors: Vec<String>,
}

fn parse(tokens: Vec<Token>) -> Parse {
    let mut tokens = tokens;
    tokens.reverse();
    Parser {
        tokens,
        builder: GreenNodeBuilder::new(),
        errors: Vec::new(),
    }
    .parse()
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::dbt_jinja2::lexer::{tokenize, Token};

    struct ParseTestCase {
        input: &'static str,
    }

    #[test]
    fn test_parse() {
        let test_cases = [
            ParseTestCase {
                input: "{% if 1 in [1,2] in [[1, 2], None] %} something {% endif %}",
            },
            ParseTestCase {
                input: "{% raw %}{% endraw %}",
            },
        ];
        for test_case in test_cases {
            parse(tokenize(test_case.input));
        }
    }

    // degenerate cases
    // '{% if 1 in [1,2] in [[1, 2], None] %} something {% endif %}'
}
