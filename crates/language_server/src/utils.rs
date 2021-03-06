use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use dbt_jinja_parser::parser::{Lang, SyntaxKind};
use tokio::fs::read;
use tower_lsp::{jsonrpc::Error, lsp_types::Url};

pub async fn read_file(path: &Path) -> Result<String, String> {
    let raw_bytes = read(path).await;
    let raw_bytes = match raw_bytes {
        Err(e) => return Err(format!("bad filesystem read: {:?}", e)),
        Ok(bytes) => bytes,
    };

    match String::from_utf8(raw_bytes) {
        Ok(res) => Ok(res),
        Err(e) => return Err(format!("couldn't read file as utf-8: {:?}", e)),
    }
}

pub fn uri_to_path(uri: &Url) -> Result<PathBuf, Error> {
    match uri.to_file_path() {
        Err(_) => Err(Error::invalid_params(
            "language server needs to run locally",
        )),
        Ok(path) => Ok(path),
    }
}

pub type SyntaxNode = rowan::SyntaxNode<Lang>;
#[allow(unused)]
pub fn print_node(node: SyntaxNode, indent: usize) {
    eprintln!("{:>indent$}{node:?}", "", node = node, indent = 2 * indent);
    node.children_with_tokens().for_each(|child| match child {
        rowan::NodeOrToken::Node(n) => print_node(n, indent + 1),
        rowan::NodeOrToken::Token(t) => {
            eprintln!(
                "{:>indent$}{node:?}",
                "",
                node = t,
                indent = 2 * (indent + 1)
            );
        }
    })
}

pub fn is_sql_file(path: &Path) -> bool {
    path.extension() == Some(OsStr::new("sql"))
}

pub enum TraverseOrder {
    Forward,
    Backward,
}

pub fn get_child_of_kind(
    node: &SyntaxNode,
    kind: SyntaxKind,
    order: TraverseOrder,
) -> Option<SyntaxNode> {
    let check_kind = |child: SyntaxNode| {
        if child.kind() == kind {
            Some(child)
        } else {
            None
        }
    };
    match order {
        TraverseOrder::Forward => node.children().filter_map(check_kind).next(),
        TraverseOrder::Backward => node.children().filter_map(check_kind).last(),
    }
}
