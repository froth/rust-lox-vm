use miette::{ByteOffset, Diagnostic, Error, LabeledSpan, NamedSource, Report, Result, SourceSpan};
use tracing::debug;

use crate::{
    chunk::Chunk,
    consume,
    gc::Gc,
    match_token,
    op::Op,
    scanner::Scanner,
    token::{Precedence, Token, TokenType},
    types::value::Value,
};

pub struct Compiler<'a, 'gc> {
    scanner: Scanner<'a>,
    eof: ByteOffset,
    chunk: Chunk,
    gc: &'gc mut Gc,
    errors: Vec<Report>,
}

#[derive(thiserror::Error, Debug, Diagnostic)]
#[error("Errors while parsing")]
pub struct ParseErrors {
    #[related]
    pub parser_errors: Vec<Report>,
}

impl<'a, 'gc> Compiler<'a, 'gc> {
    fn new(src: &'a NamedSource<String>, gc: &'gc mut Gc) -> Self {
        let eof = src.inner().len().saturating_sub(1);
        let scanner = Scanner::new(src);
        Compiler {
            scanner,
            eof,
            chunk: Chunk::new(),
            gc,
            errors: vec![],
        }
    }

    pub fn compile(src: &'a NamedSource<String>, gc: &'gc mut Gc) -> Result<Chunk> {
        let mut compiler = Compiler::new(src, gc);

        while compiler.scanner.peek().is_some() {
            compiler.declaration()?;
        }
        debug!("\n{}", compiler.chunk.disassemble(src));
        if compiler.errors.is_empty() {
            Ok(compiler.chunk)
        } else {
            Err(ParseErrors {
                parser_errors: compiler.errors,
            })?
        }
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

    fn declaration(&mut self) -> Result<()> {
        let res = self.statement();
        if let Err(err) = res {
            self.errors.push(err);
            self.synchronize();
        }
        Ok(())
    }

    fn statement(&mut self) -> Result<()> {
        if let Some(print) = match_token!(self.scanner, TokenType::Print) {
            self.print_statement(print?.location)
        } else {
            self.expression_statement()
        }
    }

    fn print_statement(&mut self, location: SourceSpan) -> Result<()> {
        self.expression()?;
        consume!(self, TokenType::Semicolon, "Expected ';' after value");

        self.chunk.write(Op::Print, location);
        Ok(())
    }

    fn expression_statement(&mut self) -> Result<()> {
        self.expression()?;
        let location = consume!(self, TokenType::Semicolon, "Expected ';' after value");
        self.chunk.write(Op::Pop, location);
        Ok(())
    }

    fn expression(&mut self) -> Result<()> {
        self.parse_precedence(Precedence::Assignment)
    }

    // parse everything at the given precedence or higher
    fn parse_precedence(&mut self, precedence: Precedence) -> Result<()> {
        let token = self.scanner.advance()?;
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
            let token = self.scanner.advance()?;
            self.infix(token)?;
        }

        Ok(())
    }

    fn advance(&mut self) -> Result<Token<'a>> {
        self.scanner.next().unwrap_or_else(|| {
            miette::bail!(
                labels = vec![LabeledSpan::at_offset(self.eof, "here")],
                "Unexpected EOF"
            )
        })
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
            TokenType::LeftParen => self.grouping()?,
            TokenType::Minus => self.unary(Op::Negate, token.location)?,
            TokenType::Bang => self.unary(Op::Not, token.location)?,
            TokenType::Number(f) => self.emit_constant(Value::Number(f), token.location),
            TokenType::Nil => self.chunk.write(Op::Nil, token.location),
            TokenType::True => self.chunk.write(Op::True, token.location),
            TokenType::False => self.chunk.write(Op::False, token.location),
            TokenType::String(s) => {
                let obj = self.gc.manage_str(s);
                self.emit_constant(Value::Obj(obj), token.location)
            }
            _ => unreachable!(), // guarded by is_prefix
        }
        Ok(())
    }

    fn infix(&mut self, token: Token) -> Result<()> {
        match token.token_type {
            TokenType::Minus => self.binary(Op::Subtract, None, Precedence::Factor, token.location),
            TokenType::Plus => self.binary(Op::Add, None, Precedence::Factor, token.location),
            TokenType::Star => self.binary(Op::Multiply, None, Precedence::Unary, token.location),
            TokenType::Slash => self.binary(Op::Divide, None, Precedence::Unary, token.location),
            TokenType::BangEqual => self.binary(
                Op::Equal,
                Some(Op::Not),
                Precedence::Comparision,
                token.location,
            ),
            TokenType::EqualEqual => {
                self.binary(Op::Equal, None, Precedence::Comparision, token.location)
            }
            TokenType::Greater => self.binary(Op::Greater, None, Precedence::Term, token.location),
            TokenType::GreaterEqual => {
                self.binary(Op::Less, Some(Op::Not), Precedence::Term, token.location)
            }
            TokenType::Less => self.binary(Op::Less, None, Precedence::Term, token.location),
            TokenType::LessEqual => {
                self.binary(Op::Greater, Some(Op::Not), Precedence::Term, token.location)
            }
            _ => unreachable!(), // guarded by infix_precedence
        }
    }

    fn binary(
        &mut self,
        op: Op,
        second_op: Option<Op>,
        precedence: Precedence,
        location: SourceSpan,
    ) -> Result<()> {
        self.parse_precedence(precedence)?;
        self.chunk.write(op, location);
        if let Some(o) = second_op {
            self.chunk.write(o, location)
        }
        Ok(())
    }

    fn unary(&mut self, op: Op, location: SourceSpan) -> Result<()> {
        self.expression()?;
        self.chunk.write(op, location);
        Ok(())
    }

    fn grouping(&mut self) -> Result<()> {
        self.expression()?;
        consume!(
            self.scanner,
            TokenType::RightParen,
            "Expected ')' after Expression"
        );
        Ok(())
    }
}
