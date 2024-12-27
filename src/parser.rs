use miette::{ByteOffset, Diagnostic, LabeledSpan, NamedSource, Report, Result, SourceSpan};
use tracing::debug;

use crate::{
    chunk::Chunk,
    compiler::Compiler,
    consume,
    gc::Gc,
    match_token,
    op::Op,
    scanner::Scanner,
    source_span_extensions::SourceSpanExtensions,
    token::{Precedence, Token, TokenType},
    types::value::Value,
};

pub struct Parser<'a, 'gc> {
    scanner: Scanner<'a>,
    eof: ByteOffset,
    chunk: Chunk,
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
            chunk: Chunk::new(),
            gc,
            errors: vec![],
            current: Compiler::new(),
        }
    }

    pub fn compile(src: &'a NamedSource<String>, gc: &'gc mut Gc) -> Result<Chunk> {
        let mut compiler = Parser::new(src, gc);

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
        let res = if let Some(var_token) = match_token!(self.scanner, TokenType::Var)? {
            self.var_declaration(var_token.location)
        } else {
            self.statement()
        };

        if let Err(err) = res {
            self.errors.push(err);
            self.synchronize();
        }
        Ok(())
    }

    fn var_declaration(&mut self, location: SourceSpan) -> Result<()> {
        let global = self.parse_variable()?;
        if (match_token!(self.scanner, TokenType::Equal)?).is_some() {
            self.expression()?;
        } else {
            self.chunk.write(Op::Nil, location);
        }
        let semicolon_location = consume!(self, TokenType::Semicolon, "Expected ';' after value");
        self.define_variable(global, location.until(semicolon_location));
        Ok(())
    }

    fn define_variable(&mut self, const_idx: u8, location: SourceSpan) {
        self.chunk.write(Op::DefineGlobal(const_idx), location);
    }

    fn parse_variable(&mut self) -> Result<u8> {
        let next = self.scanner.advance()?;
        if let TokenType::Identifier(id) = next.token_type {
            Ok(self.identifier_constant(id))
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(next.location, "here")],
                "Expected variable name but got `{}`",
                next.token_type
            )
        }
    }

    fn identifier_constant(&mut self, name: &str) -> u8 {
        self.chunk
            .add_constant(Value::Obj(self.gc.manage_str(name)))
    }

    fn statement(&mut self) -> Result<()> {
        if let Some(print) = match_token!(self.scanner, TokenType::Print)? {
            self.print_statement(print.location)
        } else if (match_token!(self.scanner, TokenType::LeftBrace)?).is_some() {
            self.current.begin_scope();
            self.block()?;
            self.current.end_scope();
            Ok(())
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

    fn block(&mut self) -> Result<()> {
        while !matches!(
            self.scanner.peek(),
            Some(Ok(Token {
                token_type: TokenType::RightBrace,
                ..
            }))
        ) {
            self.declaration()?;
        }
        consume!(self, TokenType::RightBrace, "Expected '}}' after block");
        Ok(())
    }

    fn expression(&mut self) -> Result<()> {
        self.parse_precedence(Precedence::Assignment)
    }

    // parse everything at the given precedence or higher
    fn parse_precedence(&mut self, precedence: Precedence) -> Result<()> {
        let token = self.scanner.advance()?;
        let can_assign = precedence <= Precedence::Assignment;
        if token.token_type.is_prefix() {
            self.prefix(token, can_assign)?
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

        if can_assign {
            if let Some(equal) = match_token!(self.scanner, TokenType::Equal)? {
                miette::bail!(
                    labels = vec![LabeledSpan::at(equal.location, "here")],
                    "Invalid assignment target",
                )
            }
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

    fn named_variable(&mut self, name: &str, can_assign: bool, location: SourceSpan) -> Result<()> {
        let arg = self.identifier_constant(name);
        if can_assign && match_token!(self.scanner, TokenType::Equal)?.is_some() {
            self.expression()?;
            self.chunk.write(Op::SetGlobal(arg), location);
        } else {
            self.chunk.write(Op::GetGlobal(arg), location);
        }
        Ok(())
    }

    fn prefix(&mut self, token: Token, can_assign: bool) -> Result<()> {
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
            TokenType::Identifier(name) => self.named_variable(name, can_assign, token.location)?,
            _ => unreachable!(), // guarded by is_prefix TODO: benchmark unreachable_unsafe
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
