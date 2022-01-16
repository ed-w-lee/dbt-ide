use fancy_regex::{escape, Match, Regex};
use lazy_static::lazy_static;
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
    Error,
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
        (?<!\.)                 # doesn't start with a . (for "tuple.0.0")
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
            "({})",
            {
                let mut operator_vec = Vec::from_iter(OPERATORS.iter());
                operator_vec.sort_by(|(op_a, _), (op_b, _)| {
                    op_b.len().partial_cmp(&op_a.len()).unwrap()
                });
                operator_vec.into_iter()
                    .map(|(&op, _)| escape(op))
                    .collect::<Vec<_>>()
                    .join("|")
            }
        ).as_ref()
    ).unwrap();

    // Regexes created by assuming static
    // - block {%%},
    // - comment {##},
    // - and variable strings {{}}
    static ref BLOCK_START_RE_STR: String = escape(r"{%").to_string();
    static ref BLOCK_END_RE_STR: String = escape(r"%}").to_string();
    static ref COMMENT_START_RE_STR: String = escape(r"{#").to_string();
    static ref COMMENT_END_RE_STR: String = escape(r"#}").to_string();
    static ref VARIABLE_START_RE_STR: String = escape(r"{{").to_string();
    static ref VARIABLE_END_RE_STR: String = escape(r"}}").to_string();
    static ref WHITESPACE_IGNORING_CONFIG_STR: String = r"(\-|\+|)".to_string();

    static ref BLOCK_START_RE: Regex = Regex::new(
        &(BLOCK_START_RE_STR.clone() + &WHITESPACE_IGNORING_CONFIG_STR)
    ).unwrap();
    static ref BLOCK_END_RE: Regex = Regex::new(
        &(WHITESPACE_IGNORING_CONFIG_STR.clone() + &BLOCK_END_RE_STR)
    ).unwrap();
    static ref COMMENT_START_RE: Regex = Regex::new(
        &(COMMENT_START_RE_STR.clone() + &WHITESPACE_IGNORING_CONFIG_STR)
    ).unwrap();
    static ref COMMENT_END_RE: Regex = Regex::new(
        &COMMENT_END_RE_STR
    ).unwrap();
    static ref VARIABLE_START_RE: Regex = Regex::new(
        &VARIABLE_START_RE_STR
    ).unwrap();
    static ref VARIABLE_END_RE: Regex = Regex::new(
        &VARIABLE_END_RE_STR
    ).unwrap();
    static ref UNKNOWN_RE: Regex = Regex::new(
        "."
    ).unwrap();

    // should search for {% raw %} and variations
    // except +%}
    static ref RAW_START_RE: Regex = Regex::new(
        format!(
            r"(?ms){0}(\-|\+|)\s*raw\s*(\-|){1}",
            *BLOCK_START_RE_STR,
            *BLOCK_END_RE_STR
        ).as_ref()
    ).unwrap();
    // should match {% endraw %} and variations
    static ref RAW_END_RE: Regex = Regex::new(
        format!(
            r"(?ms){0}(\-|\+|)\s*endraw\s*(\+|\-|){1}",
            *BLOCK_START_RE_STR,
            *BLOCK_END_RE_STR
        ).as_ref()
    ).unwrap();

    static ref EXPRESSION_RULES: Vec<Rule> = Vec::from([
        (WHITESPACE_RE.clone(), TokenKind::Whitespace, None),
        (FLOAT_RE.clone(), TokenKind::Float, None),
        (INTEGER_RE.clone(), TokenKind::Integer, None),
        (NAME_RE.clone(), TokenKind::Name, None),
        (STRING_RE.clone(), TokenKind::String, None),
        (OPERATOR_RE.clone(), TokenKind::Operator, None),
    ]);

    static ref RULES_BY_CONTEXT: HashMap<Context, (MatchType, Vec<Rule>)> =
        HashMap::from([
            (
                Context::Root,
                (
                    MatchType::Search(TokenKind::Data),
                    Vec::from([
                        (
                            RAW_START_RE.clone(),
                            TokenKind::RawBegin,
                            Some(Action::AddContext(Context::Raw)),
                        ),
                        (
                            COMMENT_START_RE.clone(),
                            TokenKind::CommentBegin,
                            Some(Action::AddContext(Context::Comment)),
                        ),
                        (
                            BLOCK_START_RE.clone(),
                            TokenKind::BlockBegin,
                            Some(Action::AddContext(Context::Block)),
                        ),
                        (
                            VARIABLE_START_RE.clone(),
                            TokenKind::VariableBegin,
                            Some(Action::AddContext(Context::Variable)),
                        ),
                    ]),
                ),
            ),
            (
                Context::Raw,
                (
                    MatchType::Search(TokenKind::Data),
                    Vec::from([(
                        RAW_END_RE.clone(),
                        TokenKind::RawEnd,
                        Some(Action::PopContext),
                    )]),
                ),
            ),
            (
                Context::Comment,
                (
                    MatchType::Search(TokenKind::Comment),
                    Vec::from([(
                        COMMENT_END_RE.clone(),
                        TokenKind::CommentEnd,
                        Some(Action::PopContext),
                    )]),
                ),
            ),
            (
                Context::Block,
                (
                    MatchType::MatchFromStart,
                    Vec::from_iter(
                        [(
                            BLOCK_END_RE.clone(),
                            TokenKind::BlockEnd,
                            Some(Action::PopContext),
                        )]
                        .into_iter()
                        .chain(EXPRESSION_RULES.clone().into_iter())
                        .chain([(
                            UNKNOWN_RE.clone(),
                            TokenKind::Error,
                            None
                        )]),
                    ),
                ),
            ),
            (
                Context::Variable,
                (
                    MatchType::MatchFromStart,
                    Vec::from_iter(
                        [(
                            VARIABLE_END_RE.clone(),
                            TokenKind::VariableEnd,
                            Some(Action::PopContext),
                        )]
                        .into_iter()
                        .chain(EXPRESSION_RULES.clone().into_iter())
                        .chain([(
                            UNKNOWN_RE.clone(),
                            TokenKind::Error,
                            None
                        )]),
                    ),
                ),
            ),
        ]);
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
enum Context {
    Root,
    Block,
    Comment,
    Variable,
    Raw,
}

impl Context {
    #[cfg(test)]
    const VALUES: [Self; 5] = [
        Self::Root,
        Self::Block,
        Self::Comment,
        Self::Variable,
        Self::Raw,
    ];
}

#[derive(Debug, Clone, Copy)]
enum Action {
    AddContext(Context),
    PopContext,
}

type Rule = (Regex, TokenKind, Option<Action>);

enum MatchType {
    /// Regexes should be searched for, and anything preceding the earliest
    /// regex should be considered to have the attached `TokenKind`
    Search(TokenKind),
    /// Regexes should match from the beginning of the string. We should take
    /// the regex that matches for the longest.
    MatchFromStart,
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
    pub fn new() -> Self {
        Lexer {}
    }

    pub fn tokenize<'a>(&self, input: &'a str) -> Vec<Token> {
        let mut context_stack = VecDeque::from([Context::Root]);
        let mut current_idx = 0;
        let mut token_stream = Vec::new();
        while current_idx != input.len() {
            let current_context = context_stack
                .back()
                .expect("Lexer failed due to empty context");
            let (match_type, rules) = RULES_BY_CONTEXT.get(current_context).unwrap();
            let matches = rules.iter().enumerate().filter_map(|(i, (reg, _, _))| {
                let reg_match = reg
                    .find_from_pos(input, current_idx)
                    .expect("regex find failed, likely because lookaheads are too complex")?;
                Some((i, reg_match.start(), reg_match.end()))
            });
            let (next_idx, maybe_action) = match match_type {
                MatchType::Search(skipped_token_kind) => {
                    match matches.min_by_key(|&(i, start_idx, _)| (start_idx, i)) {
                        Some((min_i, min_start, end_idx)) => {
                            if min_start != current_idx {
                                token_stream.push(Token {
                                    kind: *skipped_token_kind,
                                    len: min_start - current_idx,
                                });
                            }
                            let (_, token_kind, action) = rules[min_i];
                            token_stream.push(Token {
                                kind: token_kind,
                                len: end_idx - min_start,
                            });

                            (end_idx, action)
                        }
                        None => {
                            token_stream.push(Token {
                                kind: *skipped_token_kind,
                                len: input.len() - current_idx,
                            });
                            (input.len(), None)
                        }
                    }
                }
                MatchType::MatchFromStart => {
                    match matches
                        .filter(|&(_, start_idx, _)| start_idx == current_idx)
                        .max_by_key(|&(i, _, end_idx)| (end_idx, -(i as i64)))
                    {
                        Some((min_i, _, end_idx)) => {
                            let (_, token_kind, action) = rules[min_i];
                            token_stream.push(Token {
                                kind: token_kind,
                                len: end_idx - current_idx,
                            });
                            (end_idx, action)
                        }
                        None => {
                            // couldn't find anything hmm
                            unreachable!("UNKNOWN_RE should have caught the error")
                        }
                    }
                }
            };
            // update state
            if let Some(action) = maybe_action {
                match action {
                    Action::AddContext(ctx) => context_stack.push_back(ctx),
                    Action::PopContext => {
                        context_stack.pop_back();
                    }
                }
            }
            current_idx = next_idx;
        }
        token_stream
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_float_regex() {
        assert_eq!(FLOAT_RE.is_match(".1").unwrap(), false);
        assert_eq!(FLOAT_RE.is_match("0.2").unwrap(), true);
        assert_eq!(FLOAT_RE.is_match("3_4.5_6").unwrap(), true);
        assert_eq!(FLOAT_RE.is_match("30_4.50_6e-7_80_9").unwrap(), true);
        assert_eq!(FLOAT_RE.is_match("0__0.2").unwrap(), false);

        assert_eq!(FLOAT_RE.find(".2.9").unwrap().map(|m| m.end()), None);
        assert_eq!(FLOAT_RE.find(" 0.2").unwrap().map(|m| m.end()), None);
        assert_eq!(FLOAT_RE.find("0.2 test").unwrap().map(|m| m.end()), Some(3));
    }

    #[test]
    fn test_int_regex() {
        assert_eq!(INTEGER_RE.is_match("1").unwrap(), true);
        assert_eq!(INTEGER_RE.is_match("0b0_1").unwrap(), true);
        assert_eq!(INTEGER_RE.is_match("0o01234_567").unwrap(), true);
        assert_eq!(INTEGER_RE.is_match("0o8").unwrap(), true);
        assert_eq!(INTEGER_RE.is_match("0xdeadbeef888").unwrap(), true);
        assert_eq!(INTEGER_RE.is_match("0_00").unwrap(), true);

        assert_eq!(INTEGER_RE.find("02").unwrap().map(|m| m.end()), Some(1));
        assert_eq!(INTEGER_RE.find("0_1").unwrap().map(|m| m.end()), Some(1));
        assert_eq!(INTEGER_RE.find("0_02").unwrap().map(|m| m.end()), Some(3));
        assert_eq!(INTEGER_RE.find("0_00").unwrap().map(|m| m.end()), Some(4));
        assert_eq!(
            INTEGER_RE.find("23_0__0").unwrap().map(|m| m.end()),
            Some(4)
        );
    }

    #[test]
    fn test_variable_regex() {
        assert_eq!(VARIABLE_START_RE.is_match("{{").unwrap(), true);
        assert_eq!(VARIABLE_END_RE.is_match("}}").unwrap(), true);
    }

    #[test]
    fn test_raw_start_regex_search() {
        assert_eq!(
            RAW_START_RE.find("{% raw %}").unwrap().map(|m| m.start()),
            Some(0)
        );
        assert_eq!(
            RAW_START_RE
                .find("blank {% raw %}")
                .unwrap()
                .map(|m| m.start()),
            Some(6)
        );
        assert_eq!(RAW_START_RE.is_match("{%- raw %}").unwrap(), true);
        assert_eq!(RAW_START_RE.is_match("{%+ raw %}").unwrap(), true);
        assert_eq!(RAW_START_RE.is_match("{% raw -%}").unwrap(), true);
        assert_eq!(RAW_START_RE.is_match("{% raw +%}").unwrap(), false);
        assert_eq!(RAW_START_RE.is_match("{% \n raw %}").unwrap(), true);
        assert_eq!(RAW_START_RE.is_match("{% raw \n %}").unwrap(), true);
        assert_eq!(RAW_START_RE.is_match("{%- raw -%}").unwrap(), true);
        assert_eq!(RAW_START_RE.is_match("{%+ raw -%}").unwrap(), true);
        assert_eq!(RAW_START_RE.is_match("{%+ raw +%}").unwrap(), false);
        assert_eq!(RAW_START_RE.is_match("{%+ raw +%}").unwrap(), false);
    }

    #[test]
    fn test_raw_end_regex() {
        assert_eq!(
            RAW_END_RE.find("{% endraw %}").unwrap().map(|m| m.start()),
            Some(0)
        );
        assert_eq!(
            RAW_END_RE
                .find("blank {% endraw %}")
                .unwrap()
                .map(|m| m.start()),
            Some(6)
        );
        assert_eq!(RAW_END_RE.is_match("{% endraw %}").unwrap(), true);
        assert_eq!(RAW_END_RE.is_match("{%- endraw %}").unwrap(), true);
        assert_eq!(RAW_END_RE.is_match("{%+ endraw %}").unwrap(), true);
        assert_eq!(RAW_END_RE.is_match("{% endraw -%}").unwrap(), true);
        assert_eq!(RAW_END_RE.is_match("{% endraw +%}").unwrap(), true);
        assert_eq!(RAW_END_RE.is_match("{% \n endraw %}").unwrap(), true);
        assert_eq!(RAW_END_RE.is_match("{% endraw \n %}").unwrap(), true);
        assert_eq!(RAW_END_RE.is_match("{% \n endraw \n %}").unwrap(), true);
        assert_eq!(RAW_END_RE.is_match("{%- endraw -%}").unwrap(), true);
        assert_eq!(RAW_END_RE.is_match("{%+ endraw -%}").unwrap(), true);
        assert_eq!(RAW_END_RE.is_match("{%+ endraw +%}").unwrap(), true);
        assert_eq!(RAW_END_RE.is_match("{%+ endraw +%}").unwrap(), true);
    }

    #[test]
    fn test_rules_by_context_complete() {
        Context::VALUES.iter().for_each(|c| {
            RULES_BY_CONTEXT
                .get(c)
                .expect(&format!("Failed to find rules for context {:?}", c));
        })
    }

    fn print_tokenized(input: &str, tokens: &Vec<Token>) {
        let (_, to_print) = tokens.iter().fold((0, "".to_owned()), |(idx, s), token| {
            let next_idx = idx + token.len;
            let s = format!("{}\n({:?}\t {:?})", s, &input[idx..next_idx], token.kind);
            (next_idx, s)
        });
        println!("[{}\n]", to_print);
    }

    #[test]
    fn test_tokenize() {
        let lexer = Lexer::new();
        let inputs = [
            "{%+\n set something = 1 * 2 %}\n{{ something }}",
            "{% raw %}\n{{ something }} {% endraw %}",
            "{% raw %}\n{{ something }}",
            "{% ( %} {% ) %}",
            "{% { %} {% } %}",
            "{% {{ test }}",
            "{% set x = {{ test }}",
            "{{ test {% x",
            "{{ foo.0.0 }}",
            r#"{{ config(
                    materialized="table",
                    meta={
                        "something": "another", 
                    }
                ) }}"#,
        ];
        for input in inputs {
            let tokens = lexer.tokenize(input);
            print_tokenized(input, &tokens)
        }
    }
}
