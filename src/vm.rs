use miette::{Diagnostic, NamedSource, Result};
use thiserror::Error;
use tracing::debug;

use crate::{chunk::Chunk, op::Op};

#[derive(Error, Diagnostic, Debug)]
pub enum InterpreterError {
    // #[error("Oops compiler blew up")]
    // CompileError,
    #[error("Oops vm blew up")]
    RuntimeError,
}

pub struct VM;

impl VM {
    pub fn interpret<T: miette::SourceCode>(
        &self,
        chunk: Chunk,
        source: &NamedSource<T>,
    ) -> Result<()> {
        for (i, op) in chunk.code.iter().enumerate() {
            debug!("{}", chunk.disassemble_at(source, i));
            match op {
                Op::Return => return Ok(()),
                Op::Constant(index) => {
                    let constant = chunk.constants[*index as usize];
                    println!("{constant}")
                }
            }
        }
        Err(InterpreterError::RuntimeError.into())
    }
}
