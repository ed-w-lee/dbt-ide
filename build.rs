use std::{
    collections::HashMap,
    env,
    error::Error,
    fs::{read_to_string, write},
    path::{Path, PathBuf},
};

use glob::glob;
use heck::ToUpperCamelCase;
use serde::{Deserialize, Serialize};
use tera::{Context, Tera, Value};

// Copied from https://github.com/CAD97/tinyc/blob/3e9e78bd334df90ac9c956319e442140508a579f/crates/grammar/build.rs

const MANIFEST: &str = env!("CARGO_MANIFEST_DIR");
const SYNTAX_CONFIG: &str = "meta/syntax.toml";
const TEMPLATE_DIR: &str = "meta";

const SYNTAX_KINDS_SRC: &str = "syntax_kinds.rs.tera";
const TOKEN_KINDS_SRC: &str = "token_kinds.rs.tera";
const SYNTAX_KINDS_DST: &str = "syntax_kinds.rs";
const TOKEN_KINDS_DST: &str = "token_kinds.rs";

fn project_root() -> &'static Path {
    // manifest is currently at the project root
    Path::new(MANIFEST)
}

#[derive(Serialize)]
struct OperatorConfig {
    operator: String,
    name: String,
}

impl<'de> Deserialize<'de> for OperatorConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename = "OperatorConfig")]
        struct Helper(String, String);

        Helper::deserialize(deserializer).map(|helper| OperatorConfig {
            operator: helper.0,
            name: helper.1,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct SyntaxConfig {
    comparisons: Vec<OperatorConfig>,
    operators: Vec<OperatorConfig>,
    tokens: Vec<String>,
    statements: Vec<String>,
    expressions: Vec<String>,
    composites: Vec<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let root_path = project_root();
    let templates_glob = root_path.join(TEMPLATE_DIR).join("**/*.rs.tera");
    let syntax_config_path = root_path.join(SYNTAX_CONFIG);
    let out = PathBuf::from(env::var("OUT_DIR")?);

    println!(
        "cargo:rerun-if-changed={}",
        syntax_config_path.to_string_lossy()
    );
    for path in glob(&templates_glob.to_string_lossy())? {
        println!("cargo:rerun-if-changed={}", path?.to_string_lossy());
    }

    let tera = {
        let mut tera = Tera::new(&root_path.join(templates_glob).to_string_lossy())?;
        tera.register_filter(
            "camel_case",
            |value: &Value, _args: &HashMap<String, Value>| {
                let val = tera::try_get_value!("camel_case", "value", String, value);
                Ok(val.to_upper_camel_case().into())
            },
        );
        tera
    };
    let syntax_config: SyntaxConfig = toml::from_str(&read_to_string(syntax_config_path)?)?;
    let context = Context::from_serialize(syntax_config)?;

    write(
        out.join(TOKEN_KINDS_DST),
        tera.render(TOKEN_KINDS_SRC, &context)?,
    )?;
    write(
        out.join(SYNTAX_KINDS_DST),
        tera.render(SYNTAX_KINDS_SRC, &context)?,
    )?;
    Ok(())
}
