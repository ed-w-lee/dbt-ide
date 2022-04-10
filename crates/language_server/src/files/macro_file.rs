use std::path::Path;

use dbt_jinja_parser::lexer::tokenize;
use dbt_jinja_parser::parser::{parse, Parse, SyntaxKind};
use derivative::Derivative;

use crate::model::{Macro, Materialization};
use crate::position_finder::PositionFinder;
use crate::utils::{read_file, SyntaxNode};

#[derive(Derivative)]
#[derivative(Debug)]
/// This represents the metadata we need to track for a dbt macro file
pub struct MacroFile {
    pub position_finder: PositionFinder,
    #[derivative(Debug = "ignore")]
    pub parsed_repr: Parse,
    pub macros: Vec<Macro>,
    pub materializations: Vec<Materialization>,
}

impl MacroFile {
    pub fn from_file(file_contents: &str) -> Result<Self, String> {
        let parsed_repr = parse(tokenize(file_contents));
        let syntax_tree = parsed_repr.green();
        let (macros, materializations) =
            Self::macros_from_parsed(&SyntaxNode::new_root(syntax_tree.clone()));
        Ok(Self {
            position_finder: PositionFinder::from_text(file_contents),
            parsed_repr,
            macros,
            materializations,
        })
    }

    pub async fn from_file_path(file_path: &Path) -> Result<Self, String> {
        let file_contents = read_file(file_path).await?;
        Self::from_file(&file_contents)
    }

    pub fn refresh(&mut self, file_contents: &str) {
        self.position_finder = PositionFinder::from_text(file_contents);

        let parsed_repr = parse(tokenize(file_contents));
        let syntax_tree = parsed_repr.green();
        let (macros, materializations) =
            Self::macros_from_parsed(&SyntaxNode::new_root(syntax_tree.clone()));
        self.parsed_repr = parsed_repr;
        self.macros = macros;
        self.materializations = materializations;
    }

    fn macros_from_parsed(syntax_tree: &SyntaxNode) -> (Vec<Macro>, Vec<Materialization>) {
        let mut macros = Vec::new();
        let mut materializations = Vec::new();
        for some_macro in syntax_tree.descendants() {
            match some_macro.kind() {
                SyntaxKind::StmtMacro => macros.push(Self::extract_macro(&some_macro)),
                SyntaxKind::StmtMaterialization => materializations.push(Materialization {
                    name: todo!(),
                    definition: todo!(),
                }),
                _ => (),
            }
        }
        (macros, materializations)
    }

    fn get_child_of_kind(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
        node.children()
            .filter_map(|child| {
                if child.kind() == kind {
                    Some(child)
                } else {
                    None
                }
            })
            .next()
    }

    fn extract_default_arg(default_arg_node: &SyntaxNode) -> (Option<String>, Option<String>) {
        let children = default_arg_node.children_with_tokens();
        let mut seen_assign = false;
        let mut assign_target = None;
        let mut default_value = None;
        for child in children {
            eprintln!("{:?}", child);
            if !seen_assign {
                match child.kind() {
                    SyntaxKind::ExprName => {
                        assign_target = Some(child.into_node().unwrap().text().to_string())
                    }
                    SyntaxKind::Assign => seen_assign = true,
                    _ => (),
                }
            } else {
                match child.kind() {
                    SyntaxKind::Whitespace => (),
                    _ => {
                        default_value = Some(child.into_node().unwrap().text().to_string());
                        break;
                    }
                }
            }
        }
        (assign_target, default_value)
    }

    fn extract_macro(macro_node: &SyntaxNode) -> Macro {
        debug_assert!(macro_node.kind() == SyntaxKind::StmtMacro);
        let macro_start = Self::get_child_of_kind(macro_node, SyntaxKind::MacroBlockStart).unwrap();
        let name = Self::get_child_of_kind(&macro_start, SyntaxKind::ExprName)
            .map(|n| n.text().to_string());

        let signature = Self::get_child_of_kind(&macro_start, SyntaxKind::Signature);
        let mut args = Vec::new();
        let mut default_args = Vec::new();
        match signature {
            None => (),
            Some(node) => {
                for child in node.children() {
                    match child.kind() {
                        SyntaxKind::SignatureArg => {
                            match Self::get_child_of_kind(&child, SyntaxKind::ExprName) {
                                None => args.push(None),
                                Some(arg_name) => args.push(Some(arg_name.text().to_string())),
                            }
                        }
                        SyntaxKind::SignatureDefaultArg => {
                            default_args.push(Self::extract_default_arg(&child))
                        }
                        SyntaxKind::Whitespace => (),
                        _ => unreachable!(),
                    }
                }
            }
        }
        Macro {
            name,
            args,
            default_args,
        }
    }
}
