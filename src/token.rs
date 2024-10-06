use miette::SourceSpan;
use strum::Display;

#[derive(Debug, Clone, PartialEq)]
pub struct Token<'a> {
    pub token_type: TokenType<'a>,
    pub location: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Display)]
#[strum(serialize_all = "lowercase")]
pub enum TokenType<'a> {
    //single character tokens.
    #[strum(serialize = "(")]
    LeftParen,
    #[strum(serialize = ")")]
    RightParen,
    #[strum(serialize = "{")]
    LeftBrace,
    #[strum(serialize = "}")]
    RightBrace,
    #[strum(serialize = ",")]
    Comma,
    #[strum(serialize = ".")]
    Dot,
    #[strum(serialize = "-")]
    Minus,
    #[strum(serialize = "+")]
    Plus,
    #[strum(serialize = ";")]
    Semicolon,
    #[strum(serialize = "/")]
    Slash,
    #[strum(serialize = "*")]
    Star,

    // One or two character tokens.
    #[strum(serialize = "!")]
    Bang,
    #[strum(serialize = "!=")]
    BangEqual,
    #[strum(serialize = "=")]
    Equal,
    #[strum(serialize = "==")]
    EqualEqual,
    #[strum(serialize = ">")]
    Greater,
    #[strum(serialize = ">=")]
    GreaterEqual,
    #[strum(serialize = "<")]
    Less,
    #[strum(serialize = "<=")]
    LessEqual,

    // Literals
    Identifier(&'a str),
    String(&'a str),
    Number(f64),

    // Keywords.
    And,
    Class,
    Else,
    False,
    Fun,
    For,
    If,
    Nil,
    Or,
    Print,
    Return,
    Super,
    This,
    True,
    Var,
    While,
}
