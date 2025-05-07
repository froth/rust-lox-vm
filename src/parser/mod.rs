mod expression;
mod statement;

use std::{mem::replace, sync::Arc};

use miette::{ByteOffset, Diagnostic, LabeledSpan, NamedSource, Report, Result, SourceSpan};
use tracing::debug;

use crate::{
    compiler::{Compiler, FunctionType},
    gc::Gc,
    op::Op,
    scanner::Scanner,
    token::{Token, TokenType},
    types::{function::Function, obj::Obj, string::LoxString},
};

pub struct Parser<'a, 'gc> {
    scanner: Scanner<'a>,
    eof: ByteOffset,
    gc: &'gc mut Gc,
    errors: Vec<Report>,
    current: Compiler<'a>,
    src: Arc<NamedSource<String>>,
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
            current: Compiler::new(FunctionType::Script, None, Arc::new(src.clone())),
            src: Arc::new(src.clone()),
        }
    }

    pub fn compile(src: &'a NamedSource<String>, gc: &'gc mut Gc) -> Result<Obj> {
        let mut parser: Parser<'_, '_> = Parser::new(src, gc);

        while parser.scanner.peek().is_some() {
            parser.declaration();
        }

        debug!("\n{}", parser.current.chunk.disassemble());
        if parser.errors.is_empty() {
            Ok(Obj::Function(
                parser.end_compiler(SourceSpan::new(parser.eof.into(), 1)),
            ))
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

    fn init_compiler(&mut self, function_type: FunctionType) {
        let name = if let FunctionType::Script = function_type {
            None
        } else {
            Some(self.scanner.previous_lexeme())
        };
        let new_compiler = Compiler::new(function_type, name, self.src.clone());
        let old_compiler = replace(&mut self.current, new_compiler);
        self.current.enclosing = Some(Box::new(old_compiler));
    }

    pub fn end_compiler(&mut self, location: SourceSpan) -> Function {
        self.current.chunk.write(Op::Nil, location);
        self.current.chunk.write(Op::Return, location);
        let arity = self.current.arity;
        let enclosing = self
            .current
            .enclosing
            .take()
            .unwrap_or(Box::new(Compiler::new(
                FunctionType::Script,
                None,
                self.src.clone(),
            )));
        let old = replace(&mut self.current, *enclosing);
        Function::new(
            arity,
            old.chunk,
            old.function_name.map(LoxString::string),
            old.upvalues,
        )
    }
}
