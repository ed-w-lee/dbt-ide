use std::{collections::VecDeque, hash::BuildHasher, ops::RangeBounds};

use super::lexer::{Token, TokenKind, COMPARE_OPERATORS};
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

    fn parse_list(&mut self) {
        todo!()
    }

    fn parse_dict(&mut self) {
        todo!()
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
                if self.error_until(TokenKind::RightParen) {
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
        todo!()
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
                        match self.current() {
                            Some(TokenKind::RightBracket) => {
                                ended_correctly = true;
                                break;
                            }
                            Some(kind) if Self::is_context_end(kind) => {
                                self.errors.push(format!(
                                    "expected ']' for subscript, but found end of context: {:?}",
                                    kind
                                ));
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
                            kind => {
                                self.bump_error();
                                self.errors.push(format!(
                                    "expected ',' for tuple or ']' for subscript, but found {:?}",
                                    kind
                                ));
                            }
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
                    self.builder.start_node(Arguments.into());
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
    fn error_until(&mut self, token: TokenKind) -> bool {
        loop {
            match self.current() {
                None => {
                    return false;
                }
                Some(t) if t == token => {
                    return true;
                }
                Some(t) if Self::is_context_end(t) => {
                    return false;
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
            ParseTestCase {
                input: "{{ 11 > 9 < 12 not in 13 }}",
            },
            ParseTestCase {
                input: "{{ 1 + 2 + 3 }}",
            },
            ParseTestCase {
                input: "{{ 1 ~ 'test' 'something' ~ blah }}",
            },
            ParseTestCase {
                input: "{{ 1 * -2 / 3 + +3 // 5 ** -3 ** 4 }}",
            },
            ParseTestCase {
                input: "{{ 1 * -2 / 3 + (+3 // 5 ** -3) ** 4 }}",
            },
            ParseTestCase {
                input: "{{ foo . 0 .blah [0] [:(1,):3, 2] }}",
            },
            ParseTestCase {
                input: "{{ foo | filter | filter2 | filt.er3 }}",
            },
            ParseTestCase {
                input: "{{ foo | filter.3 }}",
            },
            ParseTestCase {
                input: "{{ foo | filter.3 }}",
            },
            ParseTestCase {
                input: "{{ foo is divisibleby 3 is something }}",
            },
            ParseTestCase {
                input: "{{ - (1 * 2).0 is divisibleby 3 }}",
            },
            // ParseTestCase {
            //     input: "{% set else = True %}{{ 000 if 111 if 222 if else else 333"
            // }
            // ParseTestCase {
            //     input: "{% for i in 1, 2, 3 %}{{i}}{% endfor %}",
            // },
            // ParseTestCase {
            //     input: "{% if 1 in [1,2] in [[1, 2], None] %} something {% endif %}"
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
}
