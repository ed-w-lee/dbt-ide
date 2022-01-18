use std::{collections::VecDeque, hash::BuildHasher};

use super::lexer::{Token, TokenKind};
use defer_lite::defer;
use rowan::{Checkpoint, GreenNode, GreenNodeBuilder};
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

enum TupleParseMode {
    Simplified,
    WithCondExpr,
    NoCondExpr,
}

impl Parser {
    // Recursive descent

    fn parse(mut self) -> Parse {
        self.builder.start_node(Template.into());
        loop {
            match self.current() {
                None => break,
                Some(TokenKind::Data) => {
                    self.register(ExprData);
                }
                Some(TokenKind::RawBegin) => {
                    self.parse_raw();
                }
                Some(TokenKind::CommentBegin) => {
                    self.parse_comment();
                }
                Some(TokenKind::VariableBegin) => {
                    self.parse_variable();
                }
                Some(TokenKind::BlockBegin) => {}
                Some(t) => {
                    println!("{:?}", t);
                }
            }
        }

        self.builder.finish_node();
        Parse {
            green_node: self.builder.finish(),
            errors: self.errors,
        }
    }

    fn parse_compare(&mut self) {
        // TODO: placeholder
        self.skip_ws();
        self.bump();
    }

    fn parse_not(&mut self) {
        let checkpoint = self.builder.checkpoint();

        self.skip_ws();
        match self.current_tok() {
            Some(t) if t.is_name("not") => {
                self.builder.start_node_at(checkpoint, ExprNot.into());
                self.register(NameOperatorNot);
                self.parse_not();
                self.builder.finish_node();
            }
            _ => self.parse_compare(),
        }
    }

    fn parse_and(&mut self) {
        let checkpoint = self.builder.checkpoint();

        self.parse_not();
        for _ in 0.. {
            self.skip_ws();
            match self.current_tok() {
                Some(t) if t.is_name("and") => {
                    self.builder.start_node_at(checkpoint, ExprAnd.into());
                    self.register(NameOperatorAnd);
                    self.parse_not();
                    self.builder.finish_node();
                }
                _ => return,
            }
        }
    }

    fn parse_or(&mut self) {
        let checkpoint = self.builder.checkpoint();

        self.parse_and();
        for _ in 0.. {
            self.skip_ws();
            match self.current_tok() {
                Some(t) if t.is_name("or") => {
                    self.builder.start_node_at(checkpoint, ExprOr.into());
                    self.register(NameOperatorOr);
                    self.parse_and();
                    self.builder.finish_node();
                }
                _ => return,
            }
        }
    }

    fn parse_primary(&mut self) {
        todo!()
    }

    fn parse_ternary(&mut self) {
        let checkpoint = self.builder.checkpoint();

        let checkpoint2 = self.builder.checkpoint();
        self.parse_or();
        for _ in 0.. {
            self.skip_ws();
            match self.current_tok() {
                Some(t) if t.is_name("if") => {
                    self.builder.start_node_at(checkpoint2, TernaryFirst.into());
                    self.builder.finish_node();

                    self.builder.start_node_at(checkpoint, ExprTernary.into());
                    self.register(NameOperatorIf);

                    self.builder.start_node(TernaryCondition.into());
                    self.parse_or();
                    self.builder.finish_node();

                    self.skip_ws();
                    match self.current_tok() {
                        Some(t) if t.is_name("else") => {
                            self.register(NameOperatorElse);

                            self.builder.start_node(TernarySecond.into());
                            self.parse_ternary();
                            self.builder.finish_node();
                        }
                        _ => (),
                    }
                    self.builder.finish_node();
                }
                _ => return,
            }
        }
    }

    fn parse_expression(&mut self, with_condexpr: bool) {
        if with_condexpr {
            self.parse_ternary();
        } else {
            self.parse_or();
        }
    }

    fn parse_tuple(
        &mut self,
        mode: TupleParseMode,
        extra_end_rules: &[&'static str],
        explicit_parentheses: bool,
    ) {
        // checkpoint on either single expression or tuple
        let checkpoint = self.builder.checkpoint();
        let mut is_tuple = false;
        let mut count = 0;
        loop {
            self.skip_ws();
            // if we should wrap the expression as a tuple element or not
            let checkpoint2 = self.builder.checkpoint();
            match self.current_tok() {
                None => {
                    self.errors
                        .push("unexpected EOF while parsing possible tuple".into());
                    return;
                }
                Some(t) if Self::is_tuple_end(t, extra_end_rules) => {
                    break;
                }
                Some(_) => match mode {
                    TupleParseMode::Simplified => self.parse_primary(),
                    TupleParseMode::WithCondExpr => self.parse_expression(true),
                    TupleParseMode::NoCondExpr => self.parse_expression(false),
                },
            }
            count += 1;

            self.skip_ws();
            match self.current() {
                Some(TokenKind::Comma) => {
                    self.builder.start_node_at(checkpoint2, TupleElement.into());
                    self.builder.finish_node();
                    if count == 1 {
                        is_tuple = true;
                        self.builder.start_node_at(checkpoint, ExprTuple.into());
                    }
                    self.register(TupleSeparator.into());
                }
                _ => {
                    if is_tuple {
                        self.builder.start_node_at(checkpoint2, TupleElement.into());
                        self.builder.finish_node();
                    }
                    break;
                }
            }
        }

        if !is_tuple {
            match count {
                0 => {
                    if !explicit_parentheses {
                        self.errors.push("expression cannot be empty here".into());
                        return;
                    }
                    self.builder.start_node(ExprTuple.into());
                    self.skip_ws();
                    self.builder.finish_node();
                }
                1 => return,
                _ => unreachable!(),
            }
        } else {
            self.builder.finish_node();
        }
    }

    fn parse_variable(&mut self) {
        self.builder.start_node(Variable.into());
        self.bump();

        self.parse_tuple(TupleParseMode::WithCondExpr, &[], false);
        if self.error_until(TokenKind::VariableEnd) {
            self.bump();
        } else {
            self.errors
                .push("incomplete variable, expected \"}}\"".into());
        }

        self.builder.finish_node();
    }

    fn parse_comment(&mut self) {
        self.builder.start_node(Comment.into());
        self.bump();

        match self.current() {
            None => self.errors.push("incomplete comment".into()),
            Some(TokenKind::CommentData) => self.bump(),
            Some(TokenKind::CommentEnd) => {
                self.bump();
                self.builder.finish_node();
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
        self.builder.finish_node();
    }

    fn parse_raw(&mut self) {
        self.builder.start_node(StmtRaw.into());
        self.bump();

        match self.current() {
            None => self.errors.push("incomplete raw block".into()),
            Some(TokenKind::Data) => self.bump(),
            Some(TokenKind::RawEnd) => {
                self.bump();
                self.builder.finish_node();
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
        self.builder.finish_node();
    }

    // Utilities for traversing through token stream
    fn is_context_end(kind: TokenKind) -> bool {
        match kind {
            TokenKind::VariableEnd | TokenKind::BlockEnd | TokenKind::RightParen => true,
            _ => false,
        }
    }

    fn is_tuple_end(token: &Token, extra_end_rules: &[&'static str]) -> bool {
        match token.kind {
            t if Self::is_context_end(t) => true,
            TokenKind::Name => extra_end_rules.contains(&&*token.text),
            _ => false,
        }
    }

    fn current(&self) -> Option<TokenKind> {
        self.tokens.last().map(|tok| tok.kind)
    }

    fn current_tok(&self) -> Option<&Token> {
        self.tokens.last()
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

    fn register(&mut self, kind: SyntaxKind) {
        self.builder.start_node(kind.into());
        self.bump();
        self.builder.finish_node();
    }

    /// adds new tokens as syntax errors until the specified token is found.
    /// returns a boolean denoting if it successfully found the given token
    fn error_until(&mut self, token: TokenKind) -> bool {
        loop {
            match self.current() {
                None => {
                    return false;
                }
                Some(t) if t == token => {
                    return true;
                }
                Some(_) => self.bump_error(),
            }
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
    use super::{parse, Lang, Parse};
    use crate::dbt_jinja2::lexer::{tokenize, Token};

    type SyntaxNode = rowan::SyntaxNode<Lang>;

    impl Parse {
        fn syntax(&self) -> SyntaxNode {
            SyntaxNode::new_root(self.green_node.clone())
        }
    }

    fn print_node(node: SyntaxNode, indent: usize) {
        println!("{:>indent$}{node:?}", "", node = node, indent = 2 * indent);
        node.children_with_tokens().for_each(|child| match child {
            rowan::NodeOrToken::Node(n) => print_node(n, indent + 1),
            rowan::NodeOrToken::Token(t) => {
                println!(
                    "{:>indent$}{node:?}",
                    "",
                    node = t,
                    indent = 2 * (indent + 1)
                );
            }
        })
    }

    struct ParseTestCase {
        input: &'static str,
    }

    #[test]
    fn test_parse() {
        let test_cases = [
            // ParseTestCase {
            //     input: "{% if 1 in [1,2] in [[1, 2], None] %} something {% endif %}",
            // },
            ParseTestCase {
                input: "{% raw %}{% endraw %}",
            },
            ParseTestCase {
                input: "{{ 1,2, 3}} test",
            },
            ParseTestCase {
                input: "{{ 000if 111or 222if 333 if else else 444}}",
            },
            ParseTestCase {
                input: "{{ 111 and 222 or not not not 333 }}",
            },
            // ParseTestCase {
            //     input: "{% set else = True %}{{ 000 if 111 if 222 if else else 333"
            // }
            // ParseTestCase {
            //     input: "{% for i in 1, 2, 3 %}{{i}}{% endfor %}",
            // },
        ];
        for test_case in test_cases {
            let tokens = tokenize(test_case.input);
            let p = parse(tokens);
            let node = p.syntax();
            print_node(node, 0);
            println!("{:#?}", p.errors);
        }
    }

    // degenerate cases
    // '{% if 1 in [1,2] in [[1, 2], None] %} something {% endif %}'
}
