use miette::{ByteOffset, Diagnostic, LabeledSpan, NamedSource, Report, Result, SourceSpan};
use tracing::debug;

use crate::{
    compiler::{Compiler, FunctionType},
    consume,
    gc::Gc,
    match_token,
    op::Op,
    scanner::Scanner,
    source_span_extensions::SourceSpanExtensions,
    token::{Precedence, Token, TokenType},
    types::{obj::Obj, value::Value},
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
struct Jump {
    op: fn(u16) -> Op,
    location: SourceSpan,
    position: usize,
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

    fn declaration(&mut self) {
        let res = if let Ok(Some(var_token)) = match_token!(self.scanner, TokenType::Var) {
            self.var_declaration(var_token.location)
        } else {
            self.statement()
        };

        if let Err(err) = res {
            self.errors.push(err);
            self.synchronize();
        }
    }

    fn var_declaration(&mut self, location: SourceSpan) -> Result<()> {
        let global = self.parse_variable()?;
        if (match_token!(self.scanner, TokenType::Equal)?).is_some() {
            self.expression()?;
        } else {
            self.current.chunk.write(Op::Nil, location);
        }
        let semicolon_location = consume!(self, TokenType::Semicolon, "Expected ';' after value");
        self.define_variable(global, location.until(semicolon_location));
        Ok(())
    }

    fn define_variable(&mut self, global_idx: Option<u8>, location: SourceSpan) {
        if let Some(const_idx) = global_idx {
            self.current
                .chunk
                .write(Op::DefineGlobal(const_idx), location);
        } else {
            self.current.mark_latest_initialized();
        }
    }

    fn parse_variable(&mut self) -> Result<Option<u8>> {
        let next = self.scanner.advance()?;
        if let TokenType::Identifier(id) = next.token_type {
            self.declare_variable(id, next.location)?;
            if self.current.is_local() {
                Ok(None)
            } else {
                Ok(Some(self.identifier_constant(id)))
            }
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(next.location, "here")],
                "Expected variable name but got `{}`",
                next.token_type
            )
        }
    }

    fn declare_variable(&mut self, name: &'a str, location: SourceSpan) -> Result<()> {
        if self.current.is_local() {
            if self.current.has_variable_in_current_scope(name) {
                miette::bail!(
                    labels = vec![LabeledSpan::at(location, "here")],
                    "Already a variable with this name in this scope"
                )
            }
            self.current.add_local(name, location)?;
        }
        Ok(())
    }

    fn identifier_constant(&mut self, name: &str) -> u8 {
        self.current
            .chunk
            .add_constant(Value::Obj(self.gc.manage_str(name)))
    }

    fn statement(&mut self) -> Result<()> {
        if let Some(print) = match_token!(self.scanner, TokenType::Print)? {
            self.print_statement(print.location)
        } else if let Some(for_token) = match_token!(self.scanner, TokenType::For)? {
            self.for_statement(for_token.location)
        } else if let Some(if_token) = match_token!(self.scanner, TokenType::If)? {
            self.if_statement(if_token.location)
        } else if let Some(while_token) = match_token!(self.scanner, TokenType::While)? {
            self.while_statement(while_token.location)
        } else if (match_token!(self.scanner, TokenType::LeftBrace)?).is_some() {
            self.begin_scope();
            let closing_location = self.block()?;
            self.end_scope(closing_location);
            Ok(())
        } else {
            self.expression_statement()
        }
    }

    fn print_statement(&mut self, location: SourceSpan) -> Result<()> {
        self.expression()?;
        consume!(self, TokenType::Semicolon, "Expected ';' after value");

        self.current.chunk.write(Op::Print, location);
        Ok(())
    }

    fn if_statement(&mut self, location: SourceSpan) -> Result<()> {
        consume!(self, TokenType::LeftParen, "Expected '(' after if");
        self.expression()?;
        let right_paren_location =
            consume!(self, TokenType::RightParen, "Expected ')' after condition");

        let location = location.until(right_paren_location);
        let then_jump = self.emit_jump(Op::JumpIfFalse, location);
        self.current.chunk.write(Op::Pop, location);

        self.statement()?;

        let else_jump = self.emit_jump(Op::Jump, location);

        self.patch_jump(then_jump)?;
        self.current.chunk.write(Op::Pop, location);

        if match_token!(self.scanner, TokenType::Else)?.is_some() {
            self.statement()?;
        }
        self.patch_jump(else_jump)?;

        Ok(())
    }

    fn while_statement(&mut self, location: SourceSpan) -> Result<()> {
        let loop_start = self.current.chunk.code.len();
        consume!(self, TokenType::LeftParen, "Expected '(' after while");
        self.expression()?;
        let right_paren_location =
            consume!(self, TokenType::RightParen, "Expected ')' after condition");
        let location = location.until(right_paren_location);

        let exit_jump = self.emit_jump(Op::JumpIfFalse, location);
        self.current.chunk.write(Op::Pop, location);

        self.statement()?;

        self.emit_loop(loop_start, location)?;

        self.patch_jump(exit_jump)?;
        self.current.chunk.write(Op::Pop, location);
        Ok(())
    }

    fn for_statement(&mut self, location: SourceSpan) -> Result<()> {
        self.begin_scope();
        consume!(self, TokenType::LeftParen, "Expected '(' after for");
        if match_token!(self.scanner, TokenType::Semicolon)?.is_some() {
            // No initializer
        } else if let Some(var) = match_token!(self.scanner, TokenType::Var)? {
            self.var_declaration(var.location)?;
        } else {
            self.expression_statement()?;
        }

        let mut loop_start = self.current.chunk.code.len();
        let mut exit_jump = None;

        if match_token!(self.scanner, TokenType::Semicolon)?.is_none() {
            self.expression()?;
            let semicolon_location = consume!(
                self,
                TokenType::Semicolon,
                "Expected ';' after loop condition"
            );
            exit_jump = Some(self.emit_jump(Op::JumpIfFalse, semicolon_location));
            self.current.chunk.write(Op::Pop, semicolon_location);
        }
        if match_token!(self.scanner, TokenType::RightParen)?.is_none() {
            let body_jump = self.emit_jump(Op::Jump, location);
            let increment_start = self.current.chunk.code.len();
            self.expression()?;
            self.current.chunk.write(Op::Pop, location);
            consume!(
                self,
                TokenType::RightParen,
                "Expected ')' after for clauses."
            );
            self.emit_loop(loop_start, location)?;
            loop_start = increment_start;
            self.patch_jump(body_jump)?;
        }

        self.statement()?;

        self.emit_loop(loop_start, location)?;
        if let Some(exit_jump) = exit_jump {
            self.patch_jump(exit_jump)?;
            self.current.chunk.write(Op::Pop, location);
        }
        self.end_scope(location);
        Ok(())
    }

    fn emit_jump(&mut self, op: fn(u16) -> Op, location: SourceSpan) -> Jump {
        let position = self.current.chunk.code.len();
        self.current.chunk.write(op(0), location);
        Jump {
            op,
            location,
            position,
        }
    }

    fn emit_loop(&mut self, loop_start: usize, location: SourceSpan) -> Result<()> {
        let jump_length = self.current.chunk.code.len() - loop_start;
        if let Ok(jump_length) = u16::try_from(jump_length) {
            self.current.chunk.write(Op::Loop(jump_length), location);
            Ok(())
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(location, "here")],
                "Loop body too large."
            )
        }
    }

    fn patch_jump(&mut self, jump: Jump) -> Result<()> {
        let jump_length = self.current.chunk.code.len() - jump.position;
        if let Ok(jump_length) = u16::try_from(jump_length) {
            self.current.chunk.code[jump.position] = (jump.op)(jump_length);
            Ok(())
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(jump.location, "here")],
                "Too much code to jump over"
            )
        }
    }

    fn expression_statement(&mut self) -> Result<()> {
        self.expression()?;
        let location = consume!(self, TokenType::Semicolon, "Expected ';' after value");
        self.current.chunk.write(Op::Pop, location);
        Ok(())
    }

    fn block(&mut self) -> Result<SourceSpan> {
        while !matches!(
            self.scanner.peek(),
            Some(Ok(Token {
                token_type: TokenType::RightBrace,
                ..
            }))
        ) && self.scanner.peek().is_some()
        {
            self.declaration();
        }
        let location = consume!(self, TokenType::RightBrace, "Expected '}}' after block");
        Ok(location)
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
        let idx = self.current.chunk.add_constant(value);
        self.current.chunk.write(Op::Constant(idx), location);
    }

    fn named_variable(&mut self, name: &str, can_assign: bool, location: SourceSpan) -> Result<()> {
        let (get_op, set_op) = if let Some(resolved) = self.current.resolve_locale(name) {
            if !resolved.initialized {
                miette::bail!(
                    labels = vec![LabeledSpan::at(location, "here")],
                    "Can't read local variable in its own initializer",
                )
            }
            let slot = resolved.slot as u8;
            (Op::GetLocal(slot), Op::SetLocal(slot))
        } else {
            let arg = self.identifier_constant(name);
            (Op::GetGlobal(arg), Op::SetGlobal(arg))
        };
        if can_assign && match_token!(self.scanner, TokenType::Equal)?.is_some() {
            self.expression()?;
            self.current.chunk.write(set_op, location);
        } else {
            self.current.chunk.write(get_op, location);
        }
        Ok(())
    }

    fn prefix(&mut self, token: Token, can_assign: bool) -> Result<()> {
        match token.token_type {
            TokenType::LeftParen => self.grouping()?,
            TokenType::Minus => self.unary(Op::Negate, token.location)?,
            TokenType::Bang => self.unary(Op::Not, token.location)?,
            TokenType::Number(f) => self.emit_constant(Value::Number(f), token.location),
            TokenType::Nil => self.current.chunk.write(Op::Nil, token.location),
            TokenType::True => self.current.chunk.write(Op::True, token.location),
            TokenType::False => self.current.chunk.write(Op::False, token.location),
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
            TokenType::And => self.and(token.location),
            TokenType::Or => self.or(token.location),
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
        self.current.chunk.write(op, location);
        if let Some(o) = second_op {
            self.current.chunk.write(o, location)
        }
        Ok(())
    }

    fn unary(&mut self, op: Op, location: SourceSpan) -> Result<()> {
        self.expression()?;
        self.current.chunk.write(op, location);
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

    fn and(&mut self, location: SourceSpan) -> Result<()> {
        let end_jump = self.emit_jump(Op::JumpIfFalse, location);
        self.current.chunk.write(Op::Pop, location);
        self.parse_precedence(Precedence::And)?;
        self.patch_jump(end_jump)?;
        Ok(())
    }

    fn or(&mut self, location: SourceSpan) -> Result<()> {
        let else_jump = self.emit_jump(Op::JumpIfFalse, location);
        let end_jump = self.emit_jump(Op::Jump, location);
        self.patch_jump(else_jump)?;
        self.current.chunk.write(Op::Pop, location);
        self.parse_precedence(Precedence::Or)?;
        self.patch_jump(end_jump)?;
        Ok(())
    }

    fn begin_scope(&mut self) {
        self.current.begin_scope();
    }

    fn end_scope(&mut self, location: SourceSpan) {
        let popped = self.current.end_scope();
        for _ in 0..popped {
            self.current.chunk.write(Op::Pop, location);
        }
    }
}
