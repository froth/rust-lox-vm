use std::{fmt::Write as _, ptr};

use miette::{Diagnostic, NamedSource, Result};
use thiserror::Error;
use tracing::debug;

use crate::{chunk::Chunk, op::Op, value::Value};

#[derive(Error, Diagnostic, Debug)]
pub enum InterpreterError {
    // #[error("Oops compiler blew up")]
    // CompileError,
    #[error("Oops vm blew up")]
    RuntimeError,
}

const STACK_SIZE: usize = 256;

pub struct VM {
    stack: Box<[Value; STACK_SIZE]>,
    stack_top: *mut Value,
}

impl VM {
    pub fn new() -> Self {
        let mut stack = Box::new([0.0; STACK_SIZE]);
        let stack_top = stack.as_mut_ptr();
        Self { stack, stack_top }
    }

    pub fn interpret<T: miette::SourceCode>(
        &mut self,
        chunk: Chunk,
        source: &NamedSource<T>,
    ) -> Result<()> {
        for (i, op) in chunk.code.iter().enumerate() {
            debug!("{}", chunk.disassemble_at(source, i));
            debug!("          {}", self.trace_stack());
            match op {
                Op::Return => {
                    println!("{}", self.pop());
                    return Ok(());
                }
                Op::Constant(index) => {
                    let constant = chunk.constants[*index as usize];
                    self.push(constant);
                }
                Op::Negate => {
                    let old = self.pop();
                    self.push(-old)
                }
            }
        }
        Err(InterpreterError::RuntimeError.into())
    }

    fn push(&mut self, value: Value) {
        // SAFETY: we have mut access to self and therefore to the stack
        unsafe { *self.stack_top = value };
        // SAFETY: NOT SAFE, stack could overflow
        unsafe { self.stack_top = self.stack_top.add(1) };
    }

    fn pop(&mut self) -> Value {
        // SAFETY: NOT SAFE, stack could overflow and underflow
        unsafe {
            self.stack_top = self.stack_top.sub(1);
            *self.stack_top
        }
    }

    fn trace_stack(&self) -> String {
        let mut res = String::new();
        let mut current = self.stack.as_ptr();
        while current != self.stack_top {
            let value = unsafe { *current };
            write!(&mut res, "[ {} ]", value).unwrap();
            unsafe { current = current.add(1) };
        }
        res
    }
}
