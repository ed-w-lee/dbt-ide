use std::path::Path;

use dbt_jinja_parser::lexer::tokenize;
use dbt_jinja_parser::parser::{parse, Parse, SyntaxKind};
use derivative::Derivative;

use crate::entity::{Macro, Materialization};
use crate::position_finder::PositionFinder;
use crate::utils::{get_child_of_kind, read_file, SyntaxNode, TraverseOrder};

#[derive(Derivative)]
#[derivative(Debug)]
/// This represents the metadata we need to track for a dbt macro file
pub struct MacroFile {
    #[derivative(Debug = "ignore")]
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
        tracing::trace!(message = "reading file contents for file path", file_path = ?file_path);
        let file_contents = read_file(file_path).await?;
        tracing::trace!(message = "finished reading file contents for file path", file_path = ?file_path);
        let to_return = Self::from_file(&file_contents);
        tracing::trace!(message = "finished parsing contents of file path", file_path = ?file_path);
        to_return
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
                SyntaxKind::StmtMaterialization => {
                    materializations.push(Self::extract_materialization(&some_macro))
                }
                _ => (),
            }
        }
        (macros, materializations)
    }

    fn extract_default_arg(
        name: &Option<String>,
        default_arg_node: &SyntaxNode,
    ) -> (Option<String>, Option<String>) {
        let children = default_arg_node.children_with_tokens();
        let mut seen_assign = false;
        let mut assign_target = None;
        let mut default_value = None;
        for child in children {
            // tracing::info!(message = "examining child", ?child, macro_name = ?name);
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
                        default_value = Some(match child {
                            rowan::NodeOrToken::Node(node) => node.text().to_string(),
                            rowan::NodeOrToken::Token(token) => token.text().to_string(),
                        });
                        break;
                    }
                }
            }
        }
        (assign_target, default_value)
    }

    fn extract_macro(macro_node: &SyntaxNode) -> Macro {
        debug_assert!(macro_node.kind() == SyntaxKind::StmtMacro);
        let macro_start = get_child_of_kind(
            macro_node,
            SyntaxKind::MacroBlockStart,
            TraverseOrder::Forward,
        )
        .unwrap();
        let name_node =
            get_child_of_kind(&macro_start, SyntaxKind::ExprName, TraverseOrder::Forward);
        let declaration_selection = match &name_node {
            None => macro_start.text_range(),
            Some(node) => node.text_range(),
        };
        let name = name_node.map(|n| n.text().to_string());

        let signature =
            get_child_of_kind(&macro_start, SyntaxKind::Signature, TraverseOrder::Forward);
        let mut args = Vec::new();
        let mut default_args = Vec::new();
        match signature {
            None => (),
            Some(node) => {
                for child in node.children() {
                    match child.kind() {
                        SyntaxKind::SignatureArg => {
                            match get_child_of_kind(
                                &child,
                                SyntaxKind::ExprName,
                                TraverseOrder::Forward,
                            ) {
                                None => args.push(None),
                                Some(arg_name) => args.push(Some(arg_name.text().to_string())),
                            }
                        }
                        SyntaxKind::SignatureDefaultArg => {
                            default_args.push(Self::extract_default_arg(&name, &child))
                        }
                        SyntaxKind::Whitespace => (),
                        _ => unreachable!(),
                    }
                }
            }
        }
        Macro {
            name,
            declaration: macro_node.text_range(),
            declaration_selection,
            args,
            default_args,
        }
    }

    fn extract_materialization(mat_node: &SyntaxNode) -> Materialization {
        debug_assert!(mat_node.kind() == SyntaxKind::StmtMaterialization);
        let mat_start = get_child_of_kind(
            mat_node,
            SyntaxKind::MaterializationBlockStart,
            TraverseOrder::Forward,
        )
        .unwrap();
        let name = get_child_of_kind(&mat_start, SyntaxKind::ExprName, TraverseOrder::Forward)
            .map(|n| n.text().to_string());

        if let Some(_) = get_child_of_kind(
            &mat_start,
            SyntaxKind::MaterializationDefault,
            TraverseOrder::Forward,
        ) {
            Materialization {
                name,
                adapter: "default".to_string(),
            }
        } else if let Some(adapter_node) = get_child_of_kind(
            &mat_start,
            SyntaxKind::MaterializationAdapter,
            TraverseOrder::Backward,
        ) {
            if let Some(str_node) = get_child_of_kind(
                &adapter_node,
                SyntaxKind::ExprConstantString,
                TraverseOrder::Forward,
            ) {
                Materialization {
                    name,
                    adapter: str_node.text().to_string(),
                }
            } else {
                Materialization {
                    name,
                    adapter: "default".to_string(),
                }
            }
        } else {
            Materialization {
                name,
                adapter: "default".to_string(),
            }
        }
    }
}
