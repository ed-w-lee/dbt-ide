/**
 * Jinja2 lexer specifically for dbt; built referencing Jinja's lexer [1]. But
 * we know dbt's Jinja environment has mostly default settings with a few extra
 * extensions [2], so we can make a few more assumptions to simplify things.  We
 * also move some of the lexing logic into parsing because we want to keep
 * complex logic in parsing.
 * [1] https://github.com/pallets/jinja/blob/11065b55a0056905a8973efec12a15dc658ef46f/src/jinja2/lexer.py
 * [2] https://github.com/dbt-labs/dbt-core/blob/e943b9fc842535e958ef4fd0b8703adc91556bc6/core/dbt/clients/jinja.py#L482
 */
use std::collections::HashMap;

#[derive(Debug)]
#[repr(u16)]
enum TokenKind {
    Add,       // "+"
    Assign,    // "="
    Colon,     // ":"
    Comma,     // ","
    Div,       // "/"
    Dot,       // "."
    Eq,        // "=="
    FloorDiv,  // "//"
    Gt,        // ">"
    GtEq,      // ">="
    LBrace,    // "{"
    LBracket,  // "["
    LParen,    // "("
    Lt,        // "<"
    LtEq,      // "<="
    Mod,       // "%"
    Mul,       // "*"
    Ne,        // "!="
    Pipe,      // "|"
    Pow,       // "**"
    RBrace,    // "}"
    RBracket,  // "]"
    RParen,    // ")"
    Semicolon, // ";"
    Sub,       // "-"
    Tilde,     // "~"

    Whitespace,
    Float,
    Integer,
    Name,
    String,
    Operator,
    BlockBegin,    // "{%" can include - or +
    BlockEnd,      // "%}" can include - or +
    VariableBegin, // "{{"
    VariableEnd,   // "}}"
    RawBegin,      // "{% raw %}" can include - or +
    RawEnd,        // "{% endraw %}" can include - or +
    CommentBegin,  // "{#"
    CommentEnd,    // "#}"
    Comment,
    Data,
    Initial,
    EOF,
    Root,
}
