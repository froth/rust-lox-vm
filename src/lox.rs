use miette::{NamedSource, Result};

use crate::{error::InterpreterError, value::Value};

pub struct Lox;

impl Lox {
    pub fn new() -> Self {
        Lox {}
    }

    pub fn run(&mut self, src: NamedSource<String>) -> Result<()> {
        let a = src.inner();
        Err(InterpreterError::RuntimeError)?
    }

    pub fn run_repl(&mut self, src: NamedSource<String>) -> Result<Option<Value>> {
        Err(InterpreterError::CompileError)?
    }
}
