use std::io::{self, Read, Result};

use dbt_jinja_parser::{
    lexer::tokenize,
    parser::{parse, print_node},
};

fn main() -> Result<()> {
    let mut buffer = String::new();
    let mut stdin = io::stdin();
    stdin.read_to_string(&mut buffer)?;
    let parse = parse(tokenize(&buffer));
    print_node(parse.syntax(), 2);
    Ok(())
}
