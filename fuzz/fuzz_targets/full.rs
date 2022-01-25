#![no_main]
use libfuzzer_sys::fuzz_target;

extern crate dbt_ide;

use dbt_ide::dbt_jinja2::{lexer::tokenize, parser::parse};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let tokens = tokenize(s);
        let p = parse(tokens);
    }
});