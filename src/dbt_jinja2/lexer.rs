use lazy_static::lazy_static;
use regex::{escape, Regex};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
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
    RawBegin,      // "{% raw %}" can include - or + (kind of)
    RawEnd,        // "{% endraw %}" can include - or +
    CommentBegin,  // "{#" can include - or +
    CommentEnd,    // "#}" can include - or +
    BlockBegin,    // "{%" can include - or +
    BlockEnd,      // "%}" can include - or +
    VariableBegin, // "{{" can include - or +
    VariableEnd,   // "}}" can include -
    Comment,
    Data,
    Initial,
    EOF,
    Root,
}

#[derive(Debug)]
struct Token {
    kind: TokenKind,
    len: usize,
}

lazy_static! {
    // Regexes copied from Jinja's lexer logic
    static ref WHITESPACE_RE: Regex = Regex::new(r"\s+").unwrap();
    static ref NEWLINE_RE: Regex = Regex::new(r"(\r\n|\r|\n)").unwrap();
    static ref STRING_RE: Regex =
        Regex::new(r#"(?s)('([^'\\]*(?:\\.[^'\\]*)*)'" r'|"([^"\\]*(?:\\.[^"\\]*)*)")"#).unwrap();
    static ref INTEGER_RE: Regex = Regex::new(
        r#"(?ix)
        (
            0b(_?[0-1])+    # binary
        |
            0o(_?[0-7])+    # octal
        |
            0x(_?[\da-f])+  # hex
        |
            [1-9](_?\d)*    # decimal
        |
            0(_?0)*         # decimal zero
        )
    "#
    )
    .unwrap();
    static ref FLOAT_RE: Regex = Regex::new(
        r#"(?ix)
        (?<!\.)                 # doesn't start with a .
        (\d+_)*\d+              # digits, possibly _ separated
        (
            (\.(\d+_)*\d+)?     # optional fractional part
            e[+\-]?(\d+_)*\d+   # exponent part
        |
            \.(\d+_)*\d+        # required fractional part
        )
    "#
    )
    .unwrap();
    // Copy-pasted from https://github.com/pallets/jinja/blob/4a33989236671e3a1b78718bc45d890616a4f75e/src/jinja2/_identifier.py
    static ref NAME_RE: Regex = Regex::new(
        r#"[\w·̀-ͯ·҃-֑҇-ׇֽֿׁׂׅׄؐ-ًؚ-ٰٟۖ-ۜ۟-۪ۤۧۨ-ܑۭܰ-݊ަ-ް߫-߽߳ࠖ-࠙ࠛ-ࠣࠥ-ࠧࠩ-࡙࠭-࡛࣓-ࣣ࣡-ःऺ-़ा-ॏ॑-ॗॢॣঁ-ঃ়া-ৄেৈো-্ৗৢৣ৾ਁ-ਃ਼ਾ-ੂੇੈੋ-੍ੑੰੱੵઁ-ઃ઼ા-ૅે-ૉો-્ૢૣૺ-૿ଁ-ଃ଼ା-ୄେୈୋ-୍ୖୗୢୣஂா-ூெ-ைொ-்ௗఀ-ఄా-ౄె-ైొ-్ౕౖౢౣಁ-ಃ಼ಾ-ೄೆ-ೈೊ-್ೕೖೢೣഀ-ഃ഻഼ാ-ൄെ-ൈൊ-്ൗൢൣංඃ්ා-ුූෘ-ෟෲෳัิ-ฺ็-๎ັິ-ູົຼ່-ໍ༹༘༙༵༷༾༿ཱ-྄྆྇ྍ-ྗྙ-ྼ࿆ါ-ှၖ-ၙၞ-ၠၢ-ၤၧ-ၭၱ-ၴႂ-ႍႏႚ-ႝ፝-፟ᜒ-᜔ᜲ-᜴ᝒᝓᝲᝳ឴-៓៝᠋-᠍ᢅᢆᢩᤠ-ᤫᤰ-᤻ᨗ-ᨛᩕ-ᩞ᩠-᩿᩼᪰-᪽ᬀ-ᬄ᬴-᭄᭫-᭳ᮀ-ᮂᮡ-ᮭ᯦-᯳ᰤ-᰷᳐-᳔᳒-᳨᳭ᳲ-᳴᳷-᳹᷀-᷹᷻-᷿‿⁀⁔⃐-⃥⃜⃡-⃰℘℮⳯-⵿⳱ⷠ-〪ⷿ-゙゚〯꙯ꙴ-꙽ꚞꚟ꛰꛱ꠂ꠆ꠋꠣ-ꠧꢀꢁꢴ-ꣅ꣠-꣱ꣿꤦ-꤭ꥇ-꥓ꦀ-ꦃ꦳-꧀ꧥꨩ-ꨶꩃꩌꩍꩻ-ꩽꪰꪲ-ꪴꪷꪸꪾ꪿꫁ꫫ-ꫯꫵ꫶ꯣ-ꯪ꯬꯭ﬞ︀-️︠-︯︳︴﹍-﹏＿𐇽𐋠𐍶-𐍺𐨁-𐨃𐨅𐨆𐨌-𐨏𐨸-𐨿𐨺𐫦𐫥𐴤-𐽆𐴧-𐽐𑀀-𑀂𑀸-𑁆𑁿-𑂂𑂰-𑂺𑄀-𑄂𑄧-𑄴𑅅𑅆𑅳𑆀-𑆂𑆳-𑇀𑇉-𑇌𑈬-𑈷𑈾𑋟-𑋪𑌀-𑌃𑌻𑌼𑌾-𑍄𑍇𑍈𑍋-𑍍𑍗𑍢𑍣𑍦-𑍬𑍰-𑍴𑐵-𑑆𑑞𑒰-𑓃𑖯-𑖵𑖸-𑗀𑗜𑗝𑘰-𑙀𑚫-𑚷𑜝-𑜫𑠬-𑠺𑨁-𑨊𑨳-𑨹𑨻-𑨾𑩇𑩑-𑩛𑪊-𑪙𑰯-𑰶𑰸-𑰿𑲒-𑲧𑲩-𑲶𑴱-𑴶𑴺𑴼𑴽𑴿-𑵅𑵇𑶊-𑶎𑶐𑶑𑶓-𑶗𑻳-𑻶𖫰-𖫴𖬰-𖬶𖽑-𖽾𖾏-𖾒𛲝𛲞𝅥-𝅩𝅭-𝅲𝅻-𝆂𝆅-𝆋𝆪-𝆭𝉂-𝉄𝨀-𝨶𝨻-𝩬𝩵𝪄𝪛-𝪟𝪡-𝪯𞀀-𞀆𞀈-𞀘𞀛-𞀡𞀣𞀤𞀦-𞣐𞀪-𞣖𞥄-𞥊󠄀-󠇯]+"#
    ).unwrap();

    // Operator-related
    static ref OPERATORS: HashMap<&'static str, TokenKind> = HashMap::from([
        ("+", TokenKind::Add),
        ("-", TokenKind::Sub),
        ("/", TokenKind::Div),
        ("//", TokenKind::FloorDiv),
        ("*", TokenKind::Mul),
        ("%", TokenKind::Mod),
        ("**", TokenKind::Pow),
        ("~", TokenKind::Tilde),
        ("[", TokenKind::LBracket),
        ("]", TokenKind::RBracket),
        ("(", TokenKind::LParen),
        (")", TokenKind::RParen),
        ("{", TokenKind::LBrace),
        ("}", TokenKind::RBrace),
        ("==", TokenKind::Eq),
        ("!=", TokenKind::Ne),
        (">", TokenKind::Gt),
        (">=", TokenKind::GtEq),
        ("<", TokenKind::Lt),
        ("<=", TokenKind::LtEq),
        ("=", TokenKind::Assign),
        (".", TokenKind::Dot),
        (":", TokenKind::Colon),
        ("|", TokenKind::Pipe),
        (",", TokenKind::Comma),
        (";", TokenKind::Semicolon),
    ]);
    static ref REVERSE_OPERATORS: HashMap<TokenKind, &'static str> = OPERATORS
        .iter()
        .map(|(&operator, &token)| (token, operator))
        .collect();
    static ref OPERATOR_RE: Regex = Regex::new(
        format!(
            "{}",
            {
                let mut operator_vec = Vec::from_iter(OPERATORS.iter());
                operator_vec.sort_by(|(op_a, _), (op_b, _)| {
                    op_b.len().partial_cmp(&op_a.len()).unwrap()
                });
                operator_vec.into_iter().map(|(&op, _)| escape(op)).collect::<Vec<_>>().join("|")
            }
        ).as_ref()
    ).unwrap();

    // Regexes created by assuming static
    // - block {%%},
    // - comment {##},
    // - and variable strings {{}}
    static ref BLOCK_START_RE_STR: String = escape(r"{%");
    static ref BLOCK_END_RE_STR: String = escape(r"%}");
    static ref COMMENT_START_RE_STR: String = escape(r"{#");
    static ref COMMENT_END_RE_STR: String = escape(r"#}");
    static ref VARIABLE_START_RE_STR: String = escape(r"{{");
    static ref VARIABLE_END_RE_STR: String = escape(r"}}");

    static ref BLOCK_START_RE: Regex = Regex::new(&BLOCK_START_RE_STR).unwrap();
    static ref BLOCK_END_RE: Regex = Regex::new(&BLOCK_END_RE_STR).unwrap();
    static ref COMMENT_START_RE: Regex = Regex::new(&COMMENT_START_RE_STR).unwrap();
    static ref COMMENT_END_RE: Regex = Regex::new(&COMMENT_END_RE_STR).unwrap();
    // TODO: check that this regex actually works
    static ref VARIABLE_START_RE: Regex = Regex::new(&VARIABLE_START_RE_STR).unwrap();
    static ref VARIABLE_END_RE: Regex = Regex::new(&VARIABLE_END_RE_STR).unwrap();

    // should match {% raw %} and variations
    // except +%}
    static ref RAW_START_RE: Regex = Regex::new(
        format!(
            r"(?ms){0}(\-|\+|)\s*raw\s*(\-|){1}",
            *BLOCK_START_RE_STR,
            *BLOCK_END_RE_STR
        ).as_ref()
    ).unwrap();
    // should match {% endraw %} and variations
    // except +%}
    static ref RAW_END_RE: Regex = Regex::new(
        format!(
            r"(?ms){0}(\-|\+|)\s*endraw\s*(\+|\-|){1}",
            *BLOCK_START_RE_STR,
            *BLOCK_END_RE_STR
        ).as_ref()
    ).unwrap();
}

#[derive(Debug)]
enum BlockContext {
    Root,
    Block,
    Comment,
    Variable,
    Raw,
}

/// Manages state that lexer uses to tokenize.
/// balancing brackets shouldn't be done in the lexer
#[derive(Debug)]
struct TokenizeState {
    context_stack: VecDeque<BlockContext>,
}

/// Jinja2 lexer specifically for dbt
///
/// Built referencing
/// [Jinja's lexer](https://github.com/pallets/jinja/blob/11065b55a0056905a8973efec12a15dc658ef46f/src/jinja2/lexer.py).
/// But we know dbt's Jinja environment has mostly default settings with a few
/// [extra extensions](https://github.com/dbt-labs/dbt-core/blob/e943b9fc842535e958ef4fd0b8703adc91556bc6/core/dbt/clients/jinja.py#L482),
/// so we can make a few more assumptions to simplify things.
///
/// We want to use this for an IDE, so we need to handle partial parses. For
/// example, handling cases like:
/// ```jinja
/// {% ( %} {% ) %}
/// {% { %} {% } %}
/// {% {{ test }}
/// {% set x = {{ test }}
/// {{ test {% x
/// ```
/// My current attitude is to just assume that block delimiters aren't going to
/// be used raw inside other blocks since '%' = mod and '}' = end of dict. Thus,
/// finding them should signify an incomplete expression inside the block.
///
/// The original lexer some ambiguity these by maintaining
/// [a stack of open and close braces](https://github.com/pallets/jinja/blob/11065b55a0056905a8973efec12a15dc658ef46f/src/jinja2/lexer.py#L713),
/// but that's to support custom delimiters.
#[derive(Debug)]
struct Lexer {}

impl Lexer {
    fn tokenize<'a>(&self, input: &'a str) -> Vec<Token> {
        let state = TokenizeState {
            context_stack: VecDeque::from([BlockContext::Root]),
        };

        todo!()
    }

    fn next_token(&self, input: &str) -> Token {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex() {
        println!("{:?}", *OPERATOR_RE);
        println!("{:?}", *RAW_START_RE);
    }
}
