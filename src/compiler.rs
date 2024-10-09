use std::iter::Peekable;

use miette::{LabeledSpan, NamedSource, Result, SourceSpan};

use crate::{chunk::Chunk, op, scanner::Scanner};

#[derive(Debug, PartialEq, PartialOrd)]
enum Precedence {
    None,
    Assignment,
    Or,
    And,
    Equality,
    Comparision,
    Term,
    Factor,
    Unary,
    Call,
    Primary,
}

pub struct Compiler<'a> {
    scanner: Peekable<Scanner<'a>>,
    chunk: Chunk,
}

impl<'a> Compiler<'a> {
    fn new(src: &'a NamedSource<String>) -> Self {
        let scanner = Scanner::new(src);
        Compiler {
            scanner: scanner.peekable(),
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
                compiler
                    .chunk
                    .write(crate::op::Op::Return, SourceSpan::from(0..0));
                Ok(compiler.chunk)
            }
        }
    }

    fn expression(&mut self) -> Result<()> {
        let index = self.chunk.add_constant(1.3);
        self.chunk
            .write(op::Op::Constant(index), SourceSpan::from(0..0));
        Ok(())
    }
}
