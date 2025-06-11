use super::{Parser, Result};
use crate::{
    check, consume, match_token,
    op::Op,
    token::{Precedence, Token, TokenType},
    types::value::Value,
};
use miette::{miette, LabeledSpan, SourceSpan};

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
            self.infix(token, can_assign)?;
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
                let obj = self.gc.alloc(s);
                self.current.emit_constant(Value::Obj(obj), token.location)
            }
            TokenType::Identifier(name) => self.named_variable(name, can_assign, token.location)?,
            TokenType::This => self.this(token.location)?,
            _ => unreachable!(), // guarded by is_prefix TODO: benchmark unreachable_unsafe
        }
        Ok(())
    }

    fn infix(&mut self, token: Token, can_assign: bool) -> Result<()> {
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
            TokenType::LeftParen => self.call(token.location),
            TokenType::Dot => self.dot(token.location, can_assign),
            _ => unreachable!(), // guarded by infix_precedence
        }
    }

    fn this(&mut self, location: SourceSpan) -> Result<()> {
        if self.current_class.is_none() {
            miette::bail!(
                labels = vec![LabeledSpan::at(location, "here")],
                "Can't use `this` outside of a class",
            );
        }
        self.named_variable("this", false, location)
    }

    fn dot(&mut self, location: SourceSpan, can_assign: bool) -> Result<()> {
        let (name, _) = self.scanner.consume_identifier("property after .")?;

        let constant_index = self.current.identifier_constant(self.gc.alloc(name));

        if can_assign && match_token!(self.scanner, TokenType::Equal)?.is_some() {
            self.expression()?;
            self.current
                .chunk
                .write(Op::SetProperty(constant_index), location);
        } else if match_token!(self.scanner, TokenType::LeftParen)?.is_some() {
            let arg_count = self.argument_list()?;
            self.current.chunk.write(
                Op::Invoke {
                    property_index: constant_index,
                    arg_count,
                },
                location,
            );
        } else {
            self.current
                .chunk
                .write(Op::GetProperty(constant_index), location);
        }
        Ok(())
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

    fn call(&mut self, location: SourceSpan) -> Result<()> {
        let arg_count = self.argument_list()?;
        self.current.chunk.write(Op::Call(arg_count), location);
        Ok(())
    }

    fn argument_list(&mut self) -> Result<u8> {
        let mut arg_count: usize = 0;

        if !check!(self.scanner, TokenType::RightParen) {
            loop {
                self.expression()?;
                arg_count += 1;
                if match_token!(self.scanner, TokenType::Comma)?.is_none() {
                    break;
                }
            }
        }

        let closing_location = consume!(
            self.scanner,
            TokenType::RightParen,
            "Expected ')' after arguments"
        );

        u8::try_from(arg_count).map_err(|_| {
            miette!(
                labels = vec![LabeledSpan::at(closing_location, "here")],
                "Can't have more than 255 arguments.",
            )
        })
    }
}
