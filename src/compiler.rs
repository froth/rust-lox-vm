use std::iter::Peekable;

use miette::{ByteOffset, LabeledSpan, NamedSource, Result, SourceSpan};
use tracing::debug;

use crate::{
    chunk::Chunk,
    op::Op,
    scanner::Scanner,
    token::{Precedence, Token, TokenType},
    value::Value,
};

pub struct Compiler<'a> {
    scanner: Peekable<Scanner<'a>>,
    eof: ByteOffset,
    chunk: Chunk,
}

macro_rules! consume {
    ($self:ident, $pattern:pat $(if $guard:expr)?, $err_create: expr) => {{
        let token = $self.advance()?;
        match token.token_type {
            $pattern $(if $guard)? => Ok(()),
            _ => {
                #[allow(clippy::redundant_closure_call)] return Err($err_create(token));
            }
        }
    }};
}

impl<'a> Compiler<'a> {
    fn new(src: &'a NamedSource<String>) -> Self {
        let eof = src.inner().len().saturating_sub(1);
        let scanner = Scanner::new(src);
        Compiler {
            scanner: scanner.peekable(),
            eof,
            chunk: Chunk::new(),
        }
    }

    pub fn compile(src: &'a NamedSource<String>) -> Result<Chunk> {
        let mut compiler = Compiler::new(src);
        compiler.expression()?;
        match compiler.scanner.next() {
            Some(res) => {
                let token = res?;
                miette::bail!(
                    labels = vec![LabeledSpan::at(token.location, "here")],
                    "Expected end of expression but got {:?}",
                    token.token_type
                )
            }
            None => {
                compiler.chunk.write(
                    crate::op::Op::Return,
                    SourceSpan::new(compiler.eof.into(), 0),
                );
                debug!("\n{}", compiler.chunk.disassemble(src));
                Ok(compiler.chunk)
            }
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

    // parse everything at the given precedence or higher
    fn parse_precedence(&mut self, precedence: Precedence) -> Result<()> {
        let token = self.advance()?;
        if token.token_type.is_prefix() {
            self.prefix(token)?
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(token.location, "here")],
                "Expected expression but got `{}`",
                token.token_type
            )
        }

        while precedence <= self.peek_infix_precedence()? {
            let token = self.advance()?;
            self.infix(token)?;
        }

        Ok(())
    }

    fn peek_infix_precedence(&mut self) -> Result<Precedence> {
        match self.scanner.peek() {
            Some(Err(_)) => Err(self
                .scanner
                .next()
                .expect("checked some above")
                .expect_err("checked Err above")),
            Some(Ok(token)) => Ok(token.token_type.infix_precedence()),
            None => Ok(Precedence::None),
        }
    }

    fn emit_constant(&mut self, value: Value, location: SourceSpan) {
        let idx = self.chunk.add_constant(value);
        self.chunk.write(Op::Constant(idx), location);
    }

    fn prefix(&mut self, token: Token) -> Result<()> {
        match token.token_type {
            TokenType::LeftParen => self.grouping(),
            TokenType::Minus => self.unary(Op::Negate, token.location),
            TokenType::Number(f) => self.number(f, token.location),
            _ => unreachable!(), // guarded by is_prefix
        }
    }

    fn infix(&mut self, token: Token) -> Result<()> {
        match token.token_type {
            TokenType::Minus => self.binary(Op::Subtract, Precedence::Factor, token.location),
            TokenType::Plus => self.binary(Op::Add, Precedence::Factor, token.location),
            TokenType::Star => self.binary(Op::Multiply, Precedence::Unary, token.location),
            TokenType::Slash => self.binary(Op::Divide, Precedence::Unary, token.location),
            _ => unreachable!(), // guarded by infix_precedence
        }
    }

    fn binary(&mut self, op: Op, precedence: Precedence, location: SourceSpan) -> Result<()> {
        self.parse_precedence(precedence)?;
        self.chunk.write(op, location);
        Ok(())
    }

    fn unary(&mut self, op: Op, location: SourceSpan) -> Result<()> {
        self.expression()?;
        self.chunk.write(op, location);
        Ok(())
    }

    fn number(&mut self, number: f32, location: SourceSpan) -> Result<()> {
        self.emit_constant(number, location);
        Ok(())
    }

    fn expression(&mut self) -> Result<()> {
        self.parse_precedence(Precedence::Assignment)
    }

    fn grouping(&mut self) -> Result<()> {
        self.expression()?;
        consume!(
            self,
            TokenType::RightParen,
            |token: Token<'a>| miette::miette!(
                labels = vec![LabeledSpan::at(token.location, "here")],
                "Expected ')' after Expression"
            )
        )
    }
}
