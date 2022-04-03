#![no_main]
use libfuzzer_sys::fuzz_target;

extern crate dbt_jinja_parser;

use dbt_jinja_parser::{lexer::tokenize, parser::parse};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let variable = std::format!("{{{{ {:?} }}}}", s);
        let tokens = tokenize(&variable);
        let p = parse(tokens);
    }
});
