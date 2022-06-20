use std::io::{self, Read, Result};

use dbt_jinja_parser::{
    lexer::tokenize,
    parser::{parse, print_node},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "dbt_debug_tree=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer().with_ansi(false))
        .init();

    let mut buffer = String::new();
    let mut stdin = io::stdin();
    match stdin.read_to_string(&mut buffer) {
        Ok(_) => tracing::info!("finished reading stdin"),
        Err(e) => {
            tracing::error!(message = "failed to read stdin", error = ?e);
            return Err(e);
        }
    }
    let parse = parse(tokenize(&buffer));
    print!("errors: {:#?}\n", parse.get_errors());
    print_node(parse.syntax(), 2);
    Ok(())
}
