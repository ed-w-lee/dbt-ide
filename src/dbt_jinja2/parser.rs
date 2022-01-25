use core::panic;
use std::{collections::VecDeque, hash::BuildHasher, ops::RangeBounds};

use super::lexer::{Token, TokenKind, COMPARE_OPERATORS};
use defer_lite::defer;
use rowan::{Checkpoint, GreenNode, GreenNodeBuilder, TextRange};
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

// struct ParseError {
//     // text range of errors
//     range: (u32, u32),
//     message: String,
// }

struct Parser {
    tokens: Vec<Token>,
    builder: GreenNodeBuilder<'static>,
    // TODO: switch errors to use ParseError for text ranges
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
                Some(TokenKind::BlockBegin) => {
                    panic!("no work on block_begin yet");
                }
                Some(t) => {
                    panic!("unexpected top-level token: {:?}", t);
                }
            }
        }

        self.builder.finish_node();
        Parse {
            green_node: self.builder.finish(),
            errors: self.errors,
        }
    }

    // TODO: Consider re-writing recursive-descent as Pratt parser?
    // I'm currently copying Jinja's logic to avoid thinking about binding power
    // but it may be worth trying to write a Pratt parser for clarity.
    // The main risks are actually understanding Pratt parsing and figuring
    // out the binding powers for all the operators...

    /// Parses a list and adds it to the lossless AST
    ///
    /// Assumes that '[' has already been found.
    fn parse_list(&mut self) {
        self.skip_ws();
        if self.current() != Some(TokenKind::LeftBracket) {
            panic!("parse_list called while current token is not a left bracket");
        }
        self.builder.start_node(ExprList.into());
        self.bump();

        for _ in 0.. {
            self.skip_ws();
            if self.current() == Some(TokenKind::RightBracket) {
                self.bump();
                break;
            }
            self.skip_ws();
            self.parse_expression(true);

            self.skip_ws();
            match self.error_until(&[TokenKind::Comma, TokenKind::RightBracket]) {
                None => {
                    self.errors
                        .push("expected ',' or ']', but found end of context".into());
                    break;
                }
                Some(TokenKind::Comma) => {
                    self.bump();
                }
                Some(TokenKind::RightBracket) => {
                    self.bump();
                    break;
                }
                _ => unreachable!(),
            }
        }
        self.builder.finish_node();
    }

    /// Parses a dict into the lossless AST
    ///
    /// Assumes that '{' has already been found
    fn parse_dict(&mut self) {
        self.skip_ws();
        if self.current() != Some(TokenKind::LeftBrace) {
            panic!("parse_dict called while current token is not a left brace");
        }
        self.builder.start_node(ExprDict.into());
        self.bump();

        for _ in 0.. {
            self.skip_ws();
            if self.current() == Some(TokenKind::RightBrace) {
                self.bump();
                break;
            }
            self.skip_ws();

            self.builder.start_node(Pair.into());
            self.parse_expression(true);
            self.skip_ws();
            match self.current() {
                Some(TokenKind::Colon) => self.bump(),
                Some(TokenKind::RightBrace) => {
                    self.builder.finish_node();
                    self.bump();
                    self.errors
                        .push("dict key requires \": value\" to complete".into());
                    break;
                }
                Some(kind) if Self::is_expression_end(kind) => {
                    self.builder.finish_node();
                    self.errors.push(format!(
                        "expected end of dict, but found end of context {:?}",
                        kind
                    ));
                    break;
                }
                Some(kind) => {
                    self.errors
                        .push(format!("expected ':', but found {:?}", kind));
                    self.bump_error();
                }
                None => {
                    self.builder.finish_node();
                    self.errors.push("expected ':', but found EOF".into());
                    break;
                }
            }
            self.skip_ws();
            self.parse_expression(true);
            self.builder.finish_node();

            self.skip_ws();
            match self.error_until(&[TokenKind::Comma, TokenKind::RightBrace]) {
                None => {
                    self.errors
                        .push(format!("expected ',' or '}}', but found end of context"));
                    break;
                }
                Some(TokenKind::Comma) => {
                    self.bump();
                    break;
                }
                Some(TokenKind::RightBrace) => {
                    self.bump();
                    break;
                }
                _ => unreachable!(),
            }
        }
        self.builder.finish_node();
    }

    fn parse_primary(&mut self) {
        self.skip_ws();
        match self.current() {
            Some(TokenKind::Name) => {
                let current_tok = &self.current_tok().unwrap().text.as_str();
                if ["true", "false", "True", "False"].contains(current_tok) {
                    self.register(ExprConstantBool);
                } else if ["none", "None"].contains(current_tok) {
                    self.register(ExprConstantNone);
                } else {
                    self.register(ExprName);
                }
            }
            Some(TokenKind::StringLiteral) => {
                self.builder.start_node(ExprConstantString.into());
                for _ in 0.. {
                    self.skip_ws();
                    match self.current() {
                        Some(TokenKind::StringLiteral) => {
                            self.bump();
                        }
                        _ => break,
                    }
                }
                self.builder.finish_node()
            }
            Some(TokenKind::IntegerLiteral) => {
                self.bump();
            }
            Some(TokenKind::FloatLiteral) => {
                self.bump();
            }
            Some(TokenKind::LeftParen) => {
                self.builder.start_node(ExprWrapped.into());
                self.bump();
                self.parse_tuple(TupleParseMode::WithCondExpr, &[], true);
                if self.error_until(&[TokenKind::RightParen]).is_some() {
                    self.bump();
                } else {
                    self.errors
                        .push("expected ')' before end of context".into());
                }
                self.builder.finish_node();
            }
            Some(TokenKind::LeftBracket) => {
                self.parse_list();
            }
            Some(TokenKind::LeftBrace) => {
                self.parse_dict();
            }
            _ => self.errors.push("invalid primary expression".into()),
        }
    }

    fn parse_call_args(&mut self) {
        self.skip_ws();
        if self.current() != Some(TokenKind::LeftParen) {
            panic!("parse_call_args called while current token is not a left paren");
        }
        self.builder.start_node(CallArguments.into());
        self.bump();

        // TODO: should validating arg order be post-parse?
        let mut seen_kwarg = false;
        let mut seen_dyn_args = false;
        let mut seen_dyn_kwargs = false;

        for _ in 0.. {
            self.skip_ws();
            match self.current() {
                Some(TokenKind::RightParen) => {
                    self.bump();
                    break;
                }
                Some(TokenKind::Multiply) => {
                    if seen_dyn_args {
                        self.errors.push("multiple dynamic args found".into());
                    }
                    if seen_dyn_kwargs {
                        self.errors
                            .push("dynamic args found after dynamic kwargs".into());
                    }
                    seen_dyn_args = true;

                    self.builder.start_node(CallDynamicArgs.into());
                    self.bump();
                    self.skip_ws();
                    self.parse_expression(true);
                    self.builder.finish_node();
                }
                Some(TokenKind::Power) => {
                    if seen_dyn_kwargs {
                        self.errors.push("multiple dynamic kwargs found".into());
                    }
                    seen_dyn_kwargs = true;

                    self.builder.start_node(CallDynamicKwargs.into());
                    self.bump();
                    self.skip_ws();
                    self.parse_expression(true);
                    self.builder.finish_node();
                }
                Some(TokenKind::Name)
                    if self.next_nonws_tok().map(|t| t.kind) == Some(TokenKind::Assign) =>
                {
                    if seen_dyn_kwargs {
                        self.errors.push("kwarg found after dynamic kwargs".into());
                    }
                    seen_kwarg = true;

                    self.builder.start_node(CallStaticKwarg.into());
                    self.bump();
                    self.skip_ws();
                    self.bump();
                    self.parse_expression(true);
                    self.builder.finish_node();
                }
                Some(kind) if Self::is_expression_end(kind) => {
                    self.errors.push(format!(
                        "incomplete call args before context end {:?}",
                        kind
                    ));
                    break;
                }
                None => break,
                Some(_) => {
                    if seen_kwarg {
                        self.errors.push("arg found after kwarg".into());
                    }
                    if seen_dyn_args {
                        self.errors.push("arg found after dynamic args".into());
                    }
                    if seen_dyn_kwargs {
                        self.errors.push("arg found after dynamic kwargs".into());
                    }

                    self.builder.start_node(CallStaticArg.into());
                    self.parse_expression(true);
                    self.builder.finish_node();
                }
            }

            match self.error_until(&[TokenKind::Comma, TokenKind::RightParen]) {
                None => {
                    self.errors
                        .push("expected ',' or ')', not end of context".into());
                    break;
                }
                Some(TokenKind::Comma) => {
                    self.bump();
                }
                Some(TokenKind::RightParen) => {
                    self.bump();
                    break;
                }
                _ => unreachable!(),
            }
        }
        self.builder.finish_node()
    }

    fn parse_call(&mut self, checkpoint: Checkpoint) {
        self.builder.start_node_at(checkpoint, ExprCall.into());
        self.parse_call_args();
        self.builder.finish_node();
    }

    fn parse_subscribed(&mut self) {
        let slice_checkpoint = self.builder.checkpoint();

        self.skip_ws();
        match self.current() {
            Some(TokenKind::Colon) => {
                self.builder
                    .start_node_at(slice_checkpoint, ExprSlice.into());
                self.bump();
            }
            _ => {
                self.parse_expression(true);
                self.skip_ws();
                match self.current() {
                    Some(kind) if kind == TokenKind::Colon => self
                        .builder
                        .start_node_at(slice_checkpoint, ExprSlice.into()),
                    _ => return,
                }
            }
        }

        self.skip_ws();
        match self.current() {
            Some(t)
                if t != TokenKind::RightBracket
                    && t != TokenKind::Comma
                    && t != TokenKind::Colon =>
            {
                self.parse_expression(true);
            }
            _ => (),
        }

        self.skip_ws();
        match self.current() {
            Some(TokenKind::Colon) => {
                self.bump();
                self.skip_ws();
                match self.current() {
                    Some(t) if t != TokenKind::RightBracket && t != TokenKind::Comma => {
                        self.parse_expression(true);
                    }
                    _ => (),
                }
            }
            _ => (),
        }

        self.builder.finish_node();
    }

    fn parse_postfix(&mut self, checkpoint: Checkpoint) {
        for _ in 0.. {
            self.skip_ws();
            match self.current() {
                Some(TokenKind::Dot) => {
                    self.bump();
                    self.skip_ws();
                    match self.current() {
                        Some(TokenKind::Name) => {
                            self.builder.start_node_at(checkpoint, ExprGetAttr.into());
                            self.register(Subscript);
                            self.builder.finish_node();
                        }
                        Some(TokenKind::IntegerLiteral) => {
                            self.builder.start_node_at(checkpoint, ExprGetItem.into());
                            self.register(Subscript);
                            self.builder.finish_node();
                        }
                        kind => {
                            self.errors.push(format!(
                                "expected name or integer as subscript, not {:?}",
                                kind
                            ));
                        }
                    }
                }
                Some(TokenKind::LeftBracket) => {
                    self.bump();
                    self.skip_ws();

                    self.builder.start_node(Subscript.into());
                    let mut ended_correctly = false;

                    let tuple_checkpoint = self.builder.checkpoint();
                    let mut is_tuple = false;

                    self.parse_subscribed();

                    for _ in 0.. {
                        self.skip_ws();
                        match self.error_until(&[TokenKind::RightBracket, TokenKind::Comma]) {
                            None => {
                                self.errors.push(format!(
                                    "expected ']' for subscript, but found end of context",
                                ));
                                break;
                            }
                            Some(TokenKind::RightBracket) => {
                                ended_correctly = true;
                                break;
                            }
                            Some(TokenKind::Comma) => {
                                self.bump();
                                if !is_tuple {
                                    self.builder
                                        .start_node_at(tuple_checkpoint, ExprTuple.into());
                                    is_tuple = true;
                                }
                                self.parse_subscribed();
                            }
                            _ => unreachable!(),
                        }
                    }

                    if is_tuple {
                        self.builder.finish_node();
                    }
                    // finish subscript node
                    self.builder.finish_node();
                    if ended_correctly {
                        self.bump();
                    }
                    self.builder.start_node_at(checkpoint, ExprGetItem.into());
                    self.builder.finish_node();
                }
                Some(TokenKind::LeftParen) => {
                    self.parse_call(checkpoint);
                }
                _ => break,
            }
        }
    }

    // Honestly, not incredibly necessary because dbt jinja doesn't have any
    // filters that have nested names...
    fn parse_nested_name(&mut self) {
        let checkpoint = self.builder.checkpoint();
        let mut is_nested = false;

        self.skip_ws();
        match self.current() {
            Some(TokenKind::Name) => self.bump(),
            kind => {
                self.errors.push(format!("expected name, not {:?}", kind));
                return;
            }
        }
        for _ in 0.. {
            self.skip_ws();
            match self.current() {
                Some(TokenKind::Dot) => {
                    if !is_nested {
                        self.builder
                            .start_node_at(checkpoint, ExprNestedName.into());
                        is_nested = true;
                    }
                    self.bump();
                    self.skip_ws();
                    match self.current() {
                        Some(TokenKind::Name) => self.bump(),
                        kind => {
                            self.errors.push(format!("expected name, not {:?}", kind));
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
        if is_nested {
            self.builder.finish_node();
        }
    }

    fn parse_filter(&mut self, checkpoint: Option<Checkpoint>, mut start_inline: bool) {
        let filter_checkpoint = match checkpoint {
            Some(c) => c,
            None => self.builder.checkpoint(),
        };

        while start_inline || self.current() == Some(TokenKind::Pipe) {
            if !start_inline {
                // must be a pipe, let's skip it
                // TODO: figure out if we want pipe in the filter node in the AST
                self.bump();
            }
            self.skip_ws();

            self.parse_nested_name();

            match self.current() {
                Some(TokenKind::LeftParen) => {
                    self.parse_call_args();
                }
                _ => (),
            }

            self.builder
                .start_node_at(filter_checkpoint, ExprFilter.into());
            self.builder.finish_node();
            start_inline = false;
        }
    }

    /// expects 'is' token that denotes test to have already been consumed
    fn parse_test(&mut self, checkpoint: Checkpoint) {
        self.skip_ws();
        let negated = match self.current_tok() {
            Some(t) if t.is_name("not") => {
                self.builder.start_node_at(checkpoint, ExprNot.into());
                self.bump();
                self.builder.start_node(ExprTest.into());
                true
            }
            _ => {
                self.builder.start_node_at(checkpoint, ExprTest.into());
                false
            }
        };

        self.parse_nested_name();

        match self.current() {
            Some(TokenKind::LeftParen) => self.parse_call_args(),
            Some(TokenKind::Name)
            | Some(TokenKind::StringLiteral)
            | Some(TokenKind::IntegerLiteral)
            | Some(TokenKind::FloatLiteral)
            | Some(TokenKind::LeftBracket)
            | Some(TokenKind::LeftBrace) => {
                let mut should_parse = true;
                if self.current() == Some(TokenKind::Name) {
                    let name = self.current_tok().unwrap().text.as_ref();
                    should_parse = match name {
                        "else" | "or" | "and" => false,
                        "is" => {
                            // Not sure why this is prohibited tbh. You can
                            // circumvent it if your test has args sooo I guess
                            // it's for clarity?
                            self.errors
                                .push("Chaining multiple tests is prohibited".into());
                            false
                        }
                        _ => true,
                    }
                }
                if should_parse {
                    self.builder.start_node(TestArguments.into());
                    {
                        let arg_checkpoint = self.builder.checkpoint();
                        self.parse_primary();
                        self.parse_postfix(arg_checkpoint);
                    }
                    self.builder.finish_node();
                }
            }
            _ => (),
        }

        if negated {
            self.builder.finish_node();
        }
        self.builder.finish_node();
    }

    fn parse_filter_expr(&mut self, checkpoint: Checkpoint) {
        for _ in 0.. {
            self.skip_ws();
            match self.current_tok() {
                Some(t) if t.kind == TokenKind::Pipe => {
                    self.parse_filter(Some(checkpoint), false);
                }
                Some(t) if t.is_name("is") => {
                    self.bump();
                    self.parse_test(checkpoint);
                }
                Some(t) if t.kind == TokenKind::LeftParen => {
                    self.parse_call(checkpoint);
                }
                _ => break,
            }
        }
    }

    fn parse_unary(&mut self, with_filter: bool) {
        let checkpoint = self.builder.checkpoint();

        self.skip_ws();
        match self.current() {
            Some(TokenKind::Subtract) => {
                self.builder.start_node_at(checkpoint, ExprNegative.into());
                self.bump();
                self.parse_unary(false);
                self.builder.finish_node();
            }
            Some(TokenKind::Add) => {
                self.builder.start_node_at(checkpoint, ExprPositive.into());
                self.bump();
                self.parse_unary(false);
                self.builder.finish_node();
            }
            _ => self.parse_primary(),
        }
        self.parse_postfix(checkpoint);
        if with_filter {
            self.parse_filter_expr(checkpoint);
        }
    }

    fn parse_pow(&mut self) {
        let checkpoint = self.builder.checkpoint();
        self.skip_ws();
        self.parse_unary(true);
        for _ in 0.. {
            self.skip_ws();
            match self.current() {
                Some(TokenKind::Power) => {
                    self.builder.start_node_at(checkpoint, ExprPower.into());
                    self.bump();
                    self.parse_unary(true);
                    self.builder.finish_node();
                }
                _ => break,
            }
        }
    }

    fn parse_math2(&mut self) {
        let checkpoint = self.builder.checkpoint();
        self.skip_ws();
        self.parse_pow();
        for _ in 0.. {
            self.skip_ws();
            match self.current() {
                Some(TokenKind::Multiply) => {
                    self.builder.start_node_at(checkpoint, ExprMultiply.into());
                    self.bump();
                    self.parse_pow();
                    self.builder.finish_node();
                }
                Some(TokenKind::Div) => {
                    self.builder.start_node_at(checkpoint, ExprDivide.into());
                    self.bump();
                    self.parse_pow();
                    self.builder.finish_node();
                }
                Some(TokenKind::FloorDiv) => {
                    self.builder
                        .start_node_at(checkpoint, ExprFloorDivide.into());
                    self.bump();
                    self.parse_pow();
                    self.builder.finish_node();
                }
                Some(TokenKind::Modulo) => {
                    self.builder.start_node_at(checkpoint, ExprModulo.into());
                    self.bump();
                    self.parse_pow();
                    self.builder.finish_node();
                }
                _ => break,
            }
        }
    }

    fn parse_concat(&mut self) {
        let checkpoint = self.builder.checkpoint();
        let mut is_concat = false;

        self.skip_ws();
        self.parse_math2();
        for _ in 0.. {
            self.skip_ws();
            match self.current() {
                Some(TokenKind::Tilde) => {
                    if !is_concat {
                        is_concat = true;
                        self.builder.start_node_at(checkpoint, ExprConcat.into());
                    }
                    self.bump();
                    self.parse_math2();
                }
                _ => break,
            }
        }
        if is_concat {
            self.builder.finish_node();
        }
    }

    fn parse_math1(&mut self) {
        let checkpoint = self.builder.checkpoint();
        self.skip_ws();
        self.parse_concat();
        for _ in 0.. {
            self.skip_ws();
            match self.current() {
                Some(TokenKind::Add) => {
                    self.builder.start_node_at(checkpoint, ExprAdd.into());
                    self.bump();
                    self.parse_concat();
                    self.builder.finish_node();
                }
                Some(TokenKind::Subtract) => {
                    self.builder.start_node_at(checkpoint, ExprSubtract.into());
                    self.bump();
                    self.parse_concat();
                    self.builder.finish_node();
                }
                _ => break,
            }
        }
    }

    fn parse_compare(&mut self) {
        let checkpoint = self.builder.checkpoint();
        let mut is_compare = false;

        self.skip_ws();
        self.parse_math1();
        for _ in 0.. {
            self.skip_ws();
            match self.current_tok() {
                Some(t) if COMPARE_OPERATORS.contains(&t.kind) => {
                    if !is_compare {
                        is_compare = true;
                        self.builder.start_node_at(checkpoint, ExprCompare.into());
                    }
                    self.builder.start_node(Operand.into());
                    self.bump();
                    self.skip_ws();
                    self.parse_math1();
                    self.builder.finish_node();
                }
                Some(t) if t.is_name("in") => {
                    if !is_compare {
                        is_compare = true;
                        self.builder.start_node_at(checkpoint, ExprCompare.into());
                    }
                    self.builder.start_node(Operand.into());
                    self.register(NameOperatorIn);
                    self.skip_ws();
                    self.parse_math1();
                    self.builder.finish_node();
                }
                Some(t) if t.is_name("not") => match self.next_nonws_tok() {
                    Some(t) if t.is_name("in") => {
                        if !is_compare {
                            is_compare = true;
                            self.builder.start_node_at(checkpoint, ExprCompare.into());
                        }
                        self.builder.start_node(Operand.into());
                        self.builder.start_node(NameOperatorNotIn.into());
                        self.bump();
                        self.skip_ws();
                        self.bump();
                        self.builder.finish_node();
                        self.parse_math1();
                        self.builder.finish_node();
                    }
                    _ => break,
                },
                _ => break,
            }
        }
        if is_compare {
            self.builder.finish_node();
        }
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
        if self.error_until(&[TokenKind::VariableEnd]).is_some() {
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
    fn is_expression_end(kind: TokenKind) -> bool {
        match kind {
            TokenKind::VariableEnd | TokenKind::BlockEnd => true,
            _ => false,
        }
    }

    fn is_tuple_end(token: &Token, extra_end_rules: &[&'static str]) -> bool {
        match token.kind {
            t if Self::is_expression_end(t) || t == TokenKind::RightParen => true,
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

    fn next_nonws_tok(&self) -> Option<&Token> {
        let mut token_iter = self.tokens.iter().rev();
        loop {
            match token_iter.next() {
                None => {
                    return None;
                }
                Some(t) if t.kind == TokenKind::Whitespace => (),
                Some(_) => {
                    break;
                }
            }
        }
        loop {
            match token_iter.next() {
                None => {
                    return None;
                }
                Some(t) if t.kind == TokenKind::Whitespace => (),
                Some(t) => {
                    return Some(t);
                }
            }
        }
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
    fn error_until(&mut self, tokens: &[TokenKind]) -> Option<TokenKind> {
        loop {
            match self.current() {
                None => return None,
                Some(t) if tokens.contains(&t) => {
                    return Some(t);
                }
                Some(kind) if Self::is_expression_end(kind) => {
                    return None;
                }
                Some(_) => self.bump_error(),
            }
        }
    }
}

pub struct Parse {
    green_node: GreenNode,
    #[allow(unused)]
    errors: Vec<String>,
}

pub fn parse(tokens: Vec<Token>) -> Parse {
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
    use crate::dbt_jinja2::lexer::tokenize;

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

    fn test_parse(test_case: ParseTestCase) {
        let tokens = tokenize(test_case.input);
        println!("{:?}", tokens);
        let p = parse(tokens);
        let node = p.syntax();
        print_node(node, 0);
        println!("{:#?}", p.errors);
    }

    macro_rules! test_case {
        ($name:ident, $input:expr) => {
            #[test]
            fn $name() {
                test_parse(ParseTestCase { input: $input });
            }
        };
    }

    test_case!(test_basic_raw, "{% raw %}raw data{% endraw %}");

    test_case!(test_tuple, "{{ 1,2, 3}} test");

    test_case!(
        test_nested_ternary,
        "{{ 000if 111or 222if 333 if else else 444}}"
    );

    test_case!(test_boolean, "{{ 111 and 222 or not not not 333 }}");

    test_case!(test_compare, "{{ 11 > 9 < 12 not in 13 }}");

    test_case!(test_math1, "{{ 1 + 2 - 3 }}");

    test_case!(test_concat, "{{ 1 ~ 'test' 'something' ~ blah }}");

    test_case!(test_math2, "{{ 1 * -2 / 3 + +3 // 5 ** -3 ** 4 }}");

    test_case!(test_primary, "{{ 1 * -2 / 3 + (+3 // 5 ** -3) ** 4 }}");

    test_case!(test_subscript, "{{ foo . 0 .blah [0] [:(1,):3, 2] }}");

    test_case!(test_slice, "{{ blah [::] }}");

    test_case!(test_slice_extra, "{{ blah [::a b c] }}");

    test_case!(test_filter, "{{ foo | filter | filter2 | filt.er3 }}");

    test_case!(test_filter_bad_nestedname, "{{ foo | filter.3 }}");

    test_case!(test_test, "{{ foo is divisibleby 3 is something }}");

    test_case!(test_test_bad_nested_is, "{{ foo is test1 is test2 }}");

    test_case!(test_test_precedence, "{{ - (1 * 2).0 is divisibleby 3 }}");

    test_case!(test_list, "{{ [1, 3, abc] }}");

    test_case!(test_list_trailing_comma, "{{ [1, 3, abc, ] }}");

    test_case!(test_list_no_end, "{{ [1, 3, abc, }}");

    test_case!(test_list_extra_tok, "{{ [1, 3, abc def, test ] }}");

    test_case!(test_list_extra_tok_no_end, "{{ [1, 3, abc def }}");

    test_case!(test_dict, "{{ {a: 1, } }}");

    test_case!(test_nested_dict, "{{ {a: {1: 2}, {2: 3}: blah} }}");

    test_case!(test_dict_no_value, "{{ {a: 1, 2 } }}");

    test_case!(test_dict_extra_key, "{{ {a: 1, 2 3: 1 } }}");

    test_case!(test_dict_extra_value, "{{ {a: 1, 2: 1 2 } }}");

    test_case!(test_dict_no_end, "{{ {a: 1, 2: ");

    test_case!(test_call, "{{ call(1, something) ");

    test_case!(
        test_call_arg_ordering,
        "{{ call(arg1, **kwargs, *args, kwarg=kw, arg2, arg3) "
    );

    test_case!(
        test_call_extra_toks,
        "{{ call(arg1 a, **kwargs b, *args c, kwarg d=kw e, arg2 f, arg3 g) "
    );

    // fuzz-generated tests

    test_case!(test_variable_dict_dict, "{{{{");

    test_case!(test_variable_dict_dict_paren, "{{{{)");

    // "{% if 1 in [1,2] in [[1, 2], None] %} something {% endif %}",
    // "{% set else = True %}{{ 000 if 111 if 222 if else else 333"
    // "{% for i in 1, 2, 3 %}{{i}}{% endfor %}",
    // "{% if 1 in [1,2] in [[1, 2], None] %} something {% endif %}"
}
