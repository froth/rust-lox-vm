use super::{Parser, Result};
use crate::{
    consume, match_token,
    op::Op,
    token::{Precedence, Token, TokenType},
    types::value::Value,
};
use miette::{LabeledSpan, SourceSpan};

impl Parser<'_, '_> {
    pub(super) fn expression(&mut self) -> Result<()> {
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
            let arg = self.current.identifier_constant(self.gc.manage_str(name));
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
            TokenType::Number(f) => self.current.emit_constant(Value::Number(f), token.location),
            TokenType::Nil => self.current.chunk.write(Op::Nil, token.location),
            TokenType::True => self.current.chunk.write(Op::True, token.location),
            TokenType::False => self.current.chunk.write(Op::False, token.location),
            TokenType::String(s) => {
                let obj = self.gc.manage_str(s);
                self.current.emit_constant(Value::Obj(obj), token.location)
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
        let end_jump = self.current.emit_jump(Op::JumpIfFalse, location);
        self.current.chunk.write(Op::Pop, location);
        self.parse_precedence(Precedence::And)?;
        self.current.patch_jump(end_jump)?;
        Ok(())
    }

    fn or(&mut self, location: SourceSpan) -> Result<()> {
        let else_jump = self.current.emit_jump(Op::JumpIfFalse, location);
        let end_jump = self.current.emit_jump(Op::Jump, location);
        self.current.patch_jump(else_jump)?;
        self.current.chunk.write(Op::Pop, location);
        self.parse_precedence(Precedence::Or)?;
        self.current.patch_jump(end_jump)?;
        Ok(())
    }
}
