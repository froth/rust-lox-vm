use crate::token::{Token, TokenType};

use std::{num::ParseFloatError, ops::DerefMut};

use miette::{Diagnostic, NamedSource, SourceSpan};

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

    #[error(transparent)]
    ParseFloatError(#[from] ParseFloatError),
}

pub type Result<T> = core::result::Result<T, ScannerError>;

pub struct Scanner<'a> {
    src: &'a NamedSource<String>,
    rest: &'a str,
    start: usize,
    at: usize,
}

impl<'a> Scanner<'a> {
    pub fn new(src: &'a NamedSource<String>) -> Self {
        Self {
            src,
            rest: src.inner().as_str(),
            start: 0,
            at: 0,
        }
    }

    pub fn next(&mut self) -> Option<Result<Token>> {
        self.start = self.at;
        let char = self.advance()?;

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
            // '"' => self.read_string(),
            // c if c.is_ascii_digit() => self.read_number(),
            // c if c.is_ascii_alphabetic() || c == '_' => Ok(Some(self.read_identifier())),
            _ => {
                return Some(Err(ScannerError::UnexpectedCharacter {
                    char,
                    src: self.src.clone(),
                    location: SourceSpan::from(self.start..self.at),
                }))
            }
        };
        Some(Ok(Token {
            token_type,
            location: SourceSpan::from(self.start..self.at),
        }))
    }

    fn advance(&mut self) -> Option<char> {
        if let Some(char) = self.rest.chars().next() {
            self.at += char.len_utf8();
            self.rest = &self.rest[1..];
            Some(char)
        } else {
            None
        }
    }

    fn matches(&mut self, expected: char) -> bool {
        match self.rest.chars().next() {
            Some(char) if char == expected => {
                self.at += char.len_utf8();
                self.rest = &self.rest[1..];
                true
            }
            _ => false,
        }
    }

    fn peek(&self) -> Option<char> {
        self.rest.chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        self.rest.chars().nth(1)
    }
}

#[cfg(test)]
mod tests {
    use miette::NamedSource;

    use crate::scanner::ScannerError;

    use super::Scanner;

    #[test]
    fn raise_error_on_unexpected_char() {
        let src = NamedSource::new("", "^".to_string());
        let mut scanner = Scanner::new(&src);
        let result = scanner.next().unwrap().unwrap_err();
        assert_matches!(result, ScannerError::UnexpectedCharacter {
             char: '^',
             src,
             location,
         } if src.name() == "" && location == (0,1).into())
    }
}
