use miette::{LabeledSpan, SourceSpan};

use super::{Parser, Result};
use crate::{
    check,
    compiler::FunctionType,
    consume, match_token,
    op::Op,
    source_span_extensions::SourceSpanExtensions,
    token::{Token, TokenType},
    types::{obj::Obj, value::Value},
};

impl Parser<'_, '_> {
    pub(super) fn declaration(&mut self) {
        let res = if let Ok(Some(class_token)) = match_token!(self.scanner, TokenType::Class) {
            self.class_declaration(class_token.location)
        } else if let Ok(Some(fun_token)) = match_token!(self.scanner, TokenType::Fun) {
            self.fun_declaration(fun_token.location)
        } else if let Ok(Some(var_token)) = match_token!(self.scanner, TokenType::Var) {
            self.var_declaration(var_token.location)
        } else {
            self.statement()
        };

        if let Err(err) = res {
            self.errors.push(err);
            self.synchronize();
        }
    }

    fn class_declaration(&mut self, location: SourceSpan) -> Result<()> {
        let next = self.scanner.advance()?;
        if let TokenType::Identifier(name) = next.token_type {
            let const_idx = self.current.identifier_constant(self.gc.alloc(name));
            self.current.declare_variable(name, location)?;
            self.current.chunk.write(Op::Class(const_idx), location);
            let var_idx = if self.current.is_local() {
                None
            } else {
                Some(const_idx)
            };
            self.current.define_variable(var_idx, location);
            consume!(
                self,
                TokenType::LeftBrace,
                "Expected '{{' before class body"
            );
            consume!(
                self,
                TokenType::RightBrace,
                "Expected '}}; after class body"
            );
            Ok(())
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(next.location, "here")],
                "Expected variable name but got `{}`",
                next.token_type
            )
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
        self.current
            .define_variable(global, location.until(semicolon_location));
        Ok(())
    }

    fn fun_declaration(&mut self, location: SourceSpan) -> Result<()> {
        let global = self.parse_variable()?;
        self.current.mark_latest_initialized();
        self.function(FunctionType::Function)?;
        self.current.define_variable(global, location);
        Ok(())
    }

    fn parse_variable(&mut self) -> Result<Option<u8>> {
        let next = self.scanner.advance()?;
        if let TokenType::Identifier(id) = next.token_type {
            self.current.declare_variable(id, next.location)?;
            if self.current.is_local() {
                Ok(None)
            } else {
                Ok(Some(self.current.identifier_constant(self.gc.alloc(id))))
            }
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(next.location, "here")],
                "Expected variable name but got `{}`",
                next.token_type
            )
        }
    }

    fn function(&mut self, function_type: FunctionType) -> Result<()> {
        self.init_compiler(function_type);
        self.current.begin_scope(); // has not to be ended because we drop the compiler in the end

        let left_paren_location = consume!(
            self,
            TokenType::LeftParen,
            "Expected '(' after function name"
        );
        if !check!(self.scanner, TokenType::RightParen) {
            loop {
                self.current.arity = if let Some(arity) = self.current.arity.checked_add(1) {
                    arity
                } else {
                    miette::bail!(
                        labels = vec![LabeledSpan::at(left_paren_location, "here")],
                        "Can't have more than 255 parameters.",
                    )
                };
                let constant = self.parse_variable()?;
                self.current.define_variable(constant, left_paren_location);
                if match_token!(self.scanner, TokenType::Comma)?.is_none() {
                    break;
                }
            }
        }
        consume!(self, TokenType::RightParen, "Expected ')' after parameters");
        consume!(
            self,
            TokenType::LeftBrace,
            "Expected '{{' before function body"
        );

        let closing_location = self.block()?;
        let function = self.end_compiler(closing_location);
        let obj_ref = self.gc.alloc(Obj::Function(function));
        let idx = self.current.chunk.add_constant(Value::Obj(obj_ref));
        self.current.chunk.write(Op::Closure(idx), closing_location);
        Ok(())
    }

    fn statement(&mut self) -> Result<()> {
        if let Some(print) = match_token!(self.scanner, TokenType::Print)? {
            self.print_statement(print.location)
        } else if let Some(for_token) = match_token!(self.scanner, TokenType::For)? {
            self.for_statement(for_token.location)
        } else if let Some(if_token) = match_token!(self.scanner, TokenType::If)? {
            self.if_statement(if_token.location)
        } else if let Some(return_token) = match_token!(self.scanner, TokenType::Return)? {
            self.return_statement(return_token.location)
        } else if let Some(while_token) = match_token!(self.scanner, TokenType::While)? {
            self.while_statement(while_token.location)
        } else if (match_token!(self.scanner, TokenType::LeftBrace)?).is_some() {
            self.current.begin_scope();
            let closing_location = self.block()?;
            self.current.end_scope(closing_location);
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
        let then_jump = self.current.emit_jump(Op::JumpIfFalse, location);
        self.current.chunk.write(Op::Pop, location);

        self.statement()?;

        let else_jump = self.current.emit_jump(Op::Jump, location);

        self.current.patch_jump(then_jump)?;
        self.current.chunk.write(Op::Pop, location);

        if match_token!(self.scanner, TokenType::Else)?.is_some() {
            self.statement()?;
        }
        self.current.patch_jump(else_jump)?;

        Ok(())
    }

    fn return_statement(&mut self, location: SourceSpan) -> Result<()> {
        if self.current.function_type == FunctionType::Script {
            miette::bail!(
                labels = vec![LabeledSpan::at(location, "here")],
                "Can't return from top-level code.",
            )
        }
        if match_token!(self.scanner, TokenType::Semicolon)?.is_some() {
            self.current.chunk.write(Op::Nil, location);
            self.current.chunk.write(Op::Return, location);
        } else {
            self.expression()?;
            consume!(
                self,
                TokenType::Semicolon,
                "Expected ';' after return value"
            );
            self.current.chunk.write(Op::Return, location);
        }
        Ok(())
    }

    fn while_statement(&mut self, location: SourceSpan) -> Result<()> {
        let loop_start = self.current.chunk.code.len();
        consume!(self, TokenType::LeftParen, "Expected '(' after while");
        self.expression()?;
        let right_paren_location =
            consume!(self, TokenType::RightParen, "Expected ')' after condition");
        let location = location.until(right_paren_location);

        let exit_jump = self.current.emit_jump(Op::JumpIfFalse, location);
        self.current.chunk.write(Op::Pop, location);

        self.statement()?;

        self.current.emit_loop(loop_start, location)?;

        self.current.patch_jump(exit_jump)?;
        self.current.chunk.write(Op::Pop, location);
        Ok(())
    }

    fn for_statement(&mut self, location: SourceSpan) -> Result<()> {
        self.current.begin_scope();
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
            exit_jump = Some(self.current.emit_jump(Op::JumpIfFalse, semicolon_location));
            self.current.chunk.write(Op::Pop, semicolon_location);
        }
        if match_token!(self.scanner, TokenType::RightParen)?.is_none() {
            let body_jump = self.current.emit_jump(Op::Jump, location);
            let increment_start = self.current.chunk.code.len();
            self.expression()?;
            self.current.chunk.write(Op::Pop, location);
            consume!(
                self,
                TokenType::RightParen,
                "Expected ')' after for clauses."
            );
            self.current.emit_loop(loop_start, location)?;
            loop_start = increment_start;
            self.current.patch_jump(body_jump)?;
        }

        self.statement()?;

        self.current.emit_loop(loop_start, location)?;
        if let Some(exit_jump) = exit_jump {
            self.current.patch_jump(exit_jump)?;
            self.current.chunk.write(Op::Pop, location);
        }
        self.current.end_scope(location);
        Ok(())
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
}
