use miette::{NamedSource, Result};

use crate::{error::InterpreterError, scanner::Scanner, value::Value};

pub struct Lox;

impl Lox {
    pub fn new() -> Self {
        Lox {}
    }

    pub fn run(&mut self, src: NamedSource<String>) -> Result<()> {
        Err(InterpreterError::RuntimeError)?
    }

    pub fn run_repl(&mut self, src: NamedSource<String>) -> Result<Option<Value>> {
        let mut scanner = Scanner::new(&src);
        while let Some(a) = scanner.next() {
            let token = a?;
            println!("{}", token.token_type)
        }
        Ok(None)
    }
}
