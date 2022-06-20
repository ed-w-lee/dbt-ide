use std::path::PathBuf;

use lazy_static::lazy_static;
use rowan::TextRange;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat, Location, Range};

fn build_args_snippet(func_name: String, arg_names: &Vec<&str>) -> String {
    let mut insert_text = func_name + "(";
    let mut i = 0;
    for arg in arg_names {
        if i > 0 {
            insert_text.push_str(", ");
        }
        i = i + 1;
        insert_text.push_str(&format!("${{{}:{}}}", i, arg));
    }
    insert_text.push(')');
    insert_text
}

#[derive(Debug, Clone)]
pub struct BuiltinMacro {
    pub name: &'static str,
    pub args: Option<Vec<&'static str>>,
    pub docs_url: &'static str,
}

lazy_static! {
    pub static ref BUILTIN_MACROS: [BuiltinMacro; 9] = [
        BuiltinMacro {
            name: "source",
            args: Some(vec!["source_name", "table_name"]),
            docs_url: "https://docs.getdbt.com/reference/dbt-jinja-functions/source",
        },
        BuiltinMacro {
            name: "env_var",
            args: Some(vec!["ENV_VAR"]),
            docs_url: "https://docs.getdbt.com/reference/dbt-jinja-functions/env_var",
        },
        BuiltinMacro {
            name: "var",
            args: Some(vec!["variable"]),
            docs_url: "https://docs.getdbt.com/reference/dbt-jinja-functions/var",
        },
        BuiltinMacro {
            name: "fromjson",
            args: Some(vec!["json_str"]),
            docs_url: "https://docs.getdbt.com/reference/dbt-jinja-functions/fromjson",
        },
        BuiltinMacro {
            name: "fromyaml",
            args: Some(vec!["yaml_str"]),
            docs_url: "https://docs.getdbt.com/reference/dbt-jinja-functions/fromyaml",
        },
        BuiltinMacro {
            name: "tojson",
            args: Some(vec!["object"]),
            docs_url: "https://docs.getdbt.com/reference/dbt-jinja-functions/tojson",
        },
        BuiltinMacro {
            name: "toyaml",
            args: Some(vec!["object"]),
            docs_url: "https://docs.getdbt.com/reference/dbt-jinja-functions/toyaml",
        },
        // special cases we should handle separately
        BuiltinMacro {
            name: "ref",
            args: None,
            docs_url: "https://docs.getdbt.com/reference/dbt-jinja-functions/ref",
        },
        BuiltinMacro {
            name: "config",
            args: None,
            docs_url: "https://docs.getdbt.com/reference/dbt-jinja-functions/config",
        },
    ];
}

impl BuiltinMacro {
    pub fn get_completion_items(&self) -> CompletionItem {
        let insert_text = {
            match &self.args {
                Some(args) => build_args_snippet(self.name.to_string(), args),
                None => self.name.to_string(),
            }
        };
        CompletionItem {
            label: self.name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            insert_text: Some(insert_text),
            detail: Some("Builtin macro".to_string()),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Macro {
    pub declaration_selection: TextRange,
    pub declaration: TextRange,
    pub name: Option<String>,
    pub args: Vec<Option<String>>,
    pub default_args: Vec<(Option<String>, Option<String>)>,
}

impl Macro {
    pub fn get_completion_items(self, package_name: Option<&str>) -> Option<CompletionItem> {
        self.name.map(|macro_name| {
            let identifier = {
                match package_name {
                    Some(package_name) => format!("{}.{}", package_name, &macro_name),
                    None => macro_name.clone(),
                }
            };
            let insert_text = build_args_snippet(
                identifier.clone(),
                &(self
                    .args
                    .iter()
                    .map(|arg| arg.as_ref().map_or("", |s| &s))
                    .collect()),
            );
            CompletionItem {
                label: identifier.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                insert_text: Some(insert_text),
                detail: Some("Macro".to_string()),
                ..Default::default()
            }
        })
    }
}
