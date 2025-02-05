use crate::token::{Token, TokenType};

use miette::{Diagnostic, LabeledSpan, NamedSource, Result, SourceSpan};

#[derive(thiserror::Error, Debug, Diagnostic)]
pub enum ScannerError {
    #[error("Unexpected character: {char}")]
    UnexpectedCharacter {
        char: char,
        #[source_code]
        src: NamedSource<String>,
        #[label("here")]
        location: SourceSpan,
    },

    #[error("Non terminated String")]
    NonTerminatedString {
        #[source_code]
        src: NamedSource<String>,
        #[label("here")]
        location: SourceSpan,
    },
}
#[macro_export]
macro_rules! consume {
    ($self:expr, $pattern:pat $(if $guard:expr)?, $err_create: expr) => {{
        let token = $self.advance().map_err(|e| e.wrap_err($err_create))?;
        match token.token_type {
            $pattern $(if $guard)? => token.location,
            _ => {
                #[allow(clippy::redundant_closure_call)] return Err(miette::miette!(
                    labels = vec![LabeledSpan::at(token.location, "here")],
                    $err_create
                ));
            }
        }
    }};
}

#[macro_export]
macro_rules! match_token {
    ($self:expr, $pattern:pat $(if $guard:expr)?) => {{
        match $self.peek() {
            Some(Err(_)) => $self.next().transpose(),
            Some(Ok(a)) => match a.token_type {
                $pattern $(if $guard)? => $self.next().transpose(),
                _ => Ok(None)
            },
            None => Ok(None),
        }
    }};
}

#[macro_export]
macro_rules! check {
    ($self:expr, $pattern:pat $(if $guard:expr)?) => {{
        match $self.peek() {
            Some(Err(_)) => false,
            Some(Ok(a)) => match a.token_type {
                $pattern $(if $guard)? => true,
                _ => false
            },
            None => false,
        }
    }};
}

pub struct Scanner<'a> {
    src: &'a NamedSource<String>,
    rest: &'a str,
    start: usize,
    at: usize,
    peeked: Option<Result<Token<'a>>>,
}

impl<'a> Scanner<'a> {
    pub fn new(src: &'a NamedSource<String>) -> Self {
        Self {
            src,
            rest: src.inner().as_str(),
            start: 0,
            at: 0,
            peeked: None,
        }
    }

    pub fn eof_offset(&self) -> usize {
        self.src.inner().len().saturating_sub(1)
    }

    pub fn peek(&mut self) -> Option<&Result<Token<'a>>> {
        if self.peeked.is_none() {
            self.peeked = self.next();
        }
        self.peeked.as_ref()
    }

    pub fn advance(&mut self) -> Result<Token<'a>> {
        self.next().unwrap_or_else(|| {
            miette::bail!(
                labels = vec![LabeledSpan::at_offset(self.eof_offset(), "here")],
                "Unexpected EOF"
            )
        })
    }

    pub fn previous_lexeme(&self) -> String {
        self.src.inner().as_str()[self.start..self.at].to_string()
    }

    fn inner_advance(&mut self) -> Option<char> {
        if let Some(char) = self.rest.chars().next() {
            self.at += char.len_utf8();
            self.rest = &self.rest[char.len_utf8()..];
            Some(char)
        } else {
            None
        }
    }

    fn matches(&mut self, expected: char) -> bool {
        match self.rest.chars().next() {
            Some(char) if char == expected => {
                self.at += char.len_utf8();
                self.rest = &self.rest[char.len_utf8()..];
                true
            }
            _ => false,
        }
    }

    fn inner_peek(&self) -> Option<char> {
        self.rest.chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        self.rest.chars().nth(1)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            if let Some(peek) = self.inner_peek() {
                match peek {
                    ' ' | '\r' | '\t' | '\n' => {
                        self.inner_advance();
                    }
                    '/' if self.peek_next() == Some('/') => self.consume_comment(),
                    _ => return,
                };
            } else {
                return;
            }
        }
    }
    fn consume_comment(&mut self) {
        while let Some(x) = self.inner_peek() {
            if x == '\n' {
                break;
            } else {
                self.inner_advance();
            }
        }
    }

    fn read_string(&mut self) -> Result<TokenType<'a>> {
        loop {
            match self.inner_peek() {
                Some('"') => break,
                Some(_) => {
                    self.inner_advance();
                }
                None => Err(ScannerError::NonTerminatedString {
                    src: self.src.clone(),
                    location: SourceSpan::from(self.start..self.at),
                })?,
            }
        }
        self.inner_advance();
        let string = &self.src.inner()[self.start + 1..self.at - 1];
        Ok(TokenType::String(string))
    }

    fn read_number(&mut self) -> TokenType<'a> {
        while self.inner_peek().is_some_and(|x| x.is_ascii_digit()) {
            self.inner_advance();
        }
        if self.inner_peek().is_some_and(|x| x == '.')
            && self.peek_next().is_some_and(|x| x.is_ascii_digit())
        {
            self.inner_advance(); // the .
            while self.inner_peek().is_some_and(|x| x.is_ascii_digit()) {
                self.inner_advance();
            }
        }
        let result = self.src.inner()[self.start..self.at]
            .parse::<f64>()
            .expect("parsing of float an not fail");
        TokenType::Number(result)
    }

    fn read_identifier(&mut self) -> TokenType<'a> {
        while self
            .inner_peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            self.inner_advance();
        }

        self.identifier_type()
    }

    fn identifier_type(&self) -> TokenType<'a> {
        let text = &self.src.inner()[self.start..self.at];
        let mut iter = text.chars();
        let next = iter.next().expect("self.start outside of token");

        match next {
            'a' if iter.as_str() == "nd" => TokenType::And,
            'c' if iter.as_str() == "lass" => TokenType::Class,
            'e' if iter.as_str() == "lse" => TokenType::Else,
            'i' if iter.as_str() == "f" => TokenType::If,
            'n' if iter.as_str() == "il" => TokenType::Nil,
            'o' if iter.as_str() == "r" => TokenType::Or,
            'p' if iter.as_str() == "rint" => TokenType::Print,
            'r' if iter.as_str() == "eturn" => TokenType::Return,
            's' if iter.as_str() == "uper" => TokenType::Super,
            'v' if iter.as_str() == "ar" => TokenType::Var,
            'w' if iter.as_str() == "hile" => TokenType::While,
            'f' => match iter.next().expect("self.start + 1 outside of token") {
                'a' if iter.as_str() == "lse" => TokenType::False,
                'o' if iter.as_str() == "r" => TokenType::For,
                'u' if iter.as_str() == "n" => TokenType::Fun,
                _ => TokenType::Identifier(text),
            },
            't' => match iter.next().expect("self.start + 1 outside of token") {
                'h' if iter.as_str() == "is" => TokenType::This,
                'r' if iter.as_str() == "ue" => TokenType::True,
                _ => TokenType::Identifier(text),
            },
            _ => TokenType::Identifier(text),
        }
    }
}

impl<'a> Iterator for Scanner<'a> {
    type Item = Result<Token<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.peeked.is_some() {
            return self.peeked.take();
        }
        self.skip_whitespace_and_comments();
        self.start = self.at;
        let char = self.inner_advance()?;

        use TokenType::*;
        let token_type = match char {
            '(' => LeftParen,
            ')' => RightParen,
            '{' => LeftBrace,
            '}' => RightBrace,
            ',' => Comma,
            '.' => Dot,
            '-' => Minus,
            '+' => Plus,
            ';' => Semicolon,
            '*' => Star,
            '!' => {
                if self.matches('=') {
                    BangEqual
                } else {
                    Bang
                }
            }
            '=' => {
                if self.matches('=') {
                    EqualEqual
                } else {
                    Equal
                }
            }
            '<' => {
                if self.matches('=') {
                    LessEqual
                } else {
                    Less
                }
            }
            '>' => {
                if self.matches('=') {
                    GreaterEqual
                } else {
                    Greater
                }
            }
            '/' => Slash,
            '"' => match self.read_string() {
                Ok(s) => s,
                Err(err) => return Some(Err(err)),
            },
            c if c.is_ascii_digit() => self.read_number(),
            c if c.is_ascii_alphabetic() || c == '_' => self.read_identifier(),
            _ => {
                return Some(Err(ScannerError::UnexpectedCharacter {
                    char,
                    src: self.src.clone(),
                    location: SourceSpan::from(self.start..self.at),
                }
                .into()))
            }
        };
        Some(Ok(Token {
            token_type,
            location: SourceSpan::from(self.start..self.at),
        }))
    }
}

#[cfg(test)]
mod tests {
    use miette::{NamedSource, SourceSpan};

    use crate::{
        scanner::ScannerError,
        token::{Token, TokenType},
    };

    use super::{Result, Scanner};
    use TokenType::*;

    #[test]
    fn scan_plus_equal_equal() {
        let src = NamedSource::new("", "+==".to_string());
        let scanner = Scanner::new(&src);
        let result: Result<Vec<Token>> = scanner.collect();
        let result = result.unwrap();
        let expected = vec![
            Token {
                token_type: Plus,
                location: SourceSpan::from(0..1),
            },
            Token {
                token_type: EqualEqual,
                location: SourceSpan::from(1..3),
            },
        ];
        assert_eq!(result, expected)
    }

    #[test]
    fn skip_whitespace() {
        let src = NamedSource::new("", "   \t\n+".to_string());
        let scanner = Scanner::new(&src);
        let result: Result<Vec<Token>> = scanner.collect();
        let result = result.unwrap();
        let expected = vec![Token {
            token_type: Plus,
            location: SourceSpan::from(5..6),
        }];
        assert_eq!(result, expected)
    }

    #[test]
    fn scan_string() {
        let src = NamedSource::new("", "\"string\"".to_string());
        let scanner = Scanner::new(&src);
        let result: Result<Vec<Token>> = scanner.collect();
        let result = result.unwrap();
        let expected = vec![Token {
            token_type: String("string"),
            location: SourceSpan::from(0..8),
        }];
        assert_eq!(result, expected)
    }

    #[test]
    fn scan_identifier() {
        let src = NamedSource::new("", "string".to_string());
        let scanner = Scanner::new(&src);
        let result: Result<Vec<Token>> = scanner.collect();
        let result = result.unwrap();
        let expected = vec![Token {
            token_type: Identifier("string"),
            location: SourceSpan::from(0..6),
        }];
        assert_eq!(result, expected)
    }

    #[test]
    fn scan_keyword() {
        let src = NamedSource::new("", "for".to_string());
        let scanner = Scanner::new(&src);
        let result: Result<Vec<Token>> = scanner.collect();
        let result = result.unwrap();
        let expected = vec![Token {
            token_type: For,
            location: SourceSpan::from(0..3),
        }];
        assert_eq!(result, expected)
    }

    #[test]
    fn scan_number() {
        let src = NamedSource::new("", "123.456".to_string());
        let scanner = Scanner::new(&src);
        let result: Result<Vec<Token>> = scanner.collect();
        let result = result.unwrap()[0].clone();
        assert_matches!(result, Token { token_type: Number(num), location } if location == SourceSpan::from(0..7) && num > 123.0)
    }

    #[test]
    fn skip_comment() {
        let src = NamedSource::new("", "//comment\n+".to_string());
        let scanner = Scanner::new(&src);
        let result: Result<Vec<Token>> = scanner.collect();
        let result = result.unwrap();
        let expected = vec![Token {
            token_type: Plus,
            location: SourceSpan::from(10..11),
        }];
        assert_eq!(result, expected)
    }

    #[test]
    fn raise_error_on_unexpected_char() {
        let src = NamedSource::new("", "^".to_string());
        let scanner = Scanner::new(&src);
        let result: Result<Vec<_>> = scanner.collect();
        let error = result.unwrap_err().downcast().unwrap();
        assert_matches!(error, ScannerError::UnexpectedCharacter {
             char: '^',
             src,
             location,
         } if src.name() == "" && location == SourceSpan::from(0..1))
    }

    #[test]
    fn raise_error_on_unterminated_string() {
        let src = NamedSource::new("", "\"unterminated".to_string());
        let scanner = Scanner::new(&src);
        let result: Result<Vec<_>> = scanner.collect();
        let error = result.unwrap_err().downcast().unwrap();
        assert_matches!(error, ScannerError::NonTerminatedString {
             src: _,
             location,
         } if location == SourceSpan::from(0..13))
    }
}
