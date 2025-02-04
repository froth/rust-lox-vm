mod expression;
mod statement;

use miette::{ByteOffset, Diagnostic, LabeledSpan, NamedSource, Report, Result, SourceSpan};
use tracing::debug;

use crate::{
    compiler::{Compiler, FunctionType},
    gc::Gc,
    op::Op,
    scanner::Scanner,
    token::{Token, TokenType},
    types::obj::Obj,
};

pub struct Parser<'a, 'gc> {
    scanner: Scanner<'a>,
    eof: ByteOffset,
    gc: &'gc mut Gc,
    errors: Vec<Report>,
    current: Compiler<'a>,
}

#[derive(thiserror::Error, Debug, Diagnostic)]
#[error("Errors while parsing")]
pub struct ParseErrors {
    #[related]
    pub parser_errors: Vec<Report>,
}

impl<'a, 'gc> Parser<'a, 'gc> {
    fn new(src: &'a NamedSource<String>, gc: &'gc mut Gc) -> Self {
        let eof = src.inner().len().saturating_sub(1);
        let scanner = Scanner::new(src);
        Parser {
            scanner,
            eof,
            gc,
            errors: vec![],
            current: Compiler::new(FunctionType::Script),
        }
    }

    pub fn compile(src: &'a NamedSource<String>, gc: &'gc mut Gc) -> Result<Obj> {
        let mut parser: Parser<'_, '_> = Parser::new(src, gc);

        while parser.scanner.peek().is_some() {
            parser.declaration();
        }
        parser
            .current
            .chunk
            .write(Op::Return, SourceSpan::new(parser.eof.into(), 1));
        debug!("\n{}", parser.current.chunk.disassemble(src));
        if parser.errors.is_empty() {
            Ok(Obj::Function(parser.current.end_compiler()))
        } else {
            Err(ParseErrors {
                parser_errors: parser.errors,
            })?
        }
    }

    fn advance(&mut self) -> Result<Token<'a>> {
        self.scanner.next().unwrap_or_else(|| {
            miette::bail!(
                labels = vec![LabeledSpan::at_offset(self.eof, "here")],
                "Unexpected EOF"
            )
        })
    }

    fn synchronize(&mut self) {
        while let Some(token) = self.scanner.peek() {
            if let Ok(token) = token {
                match token.token_type {
                    TokenType::Semicolon => {
                        let _ = self.advance();
                        return;
                    }
                    TokenType::Class
                    | TokenType::Fun
                    | TokenType::Var
                    | TokenType::For
                    | TokenType::If
                    | TokenType::While
                    | TokenType::Print => return,
                    _ => (),
                }
            }
            let _ = self.advance();
        }
    }
}
