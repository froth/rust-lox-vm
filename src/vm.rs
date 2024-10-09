use std::fmt::Write as _;

use miette::NamedSource;
use tracing::debug;

use crate::{compiler::Compiler, error::InterpreterError, op::Op, value::Value};

const STACK_SIZE: usize = 256;

pub struct VM {
    stack: Box<[Value; STACK_SIZE]>,
    stack_top: *mut Value,
}

macro_rules! binary_operator {
    ($self: ident, $op:tt) => {
        {
            let b = $self.pop();
            let a = $self.pop();
            $self.push(b $op a);
        }
    };
}

impl VM {
    pub fn new() -> Self {
        let mut stack = Box::new([0.0; STACK_SIZE]);
        let stack_top = stack.as_mut_ptr();
        Self { stack, stack_top }
    }

    pub fn interpret(
        &mut self,
        src: NamedSource<String>,
    ) -> std::result::Result<Option<Value>, InterpreterError> {
        let chunk = match Compiler::compile(&src) {
            Ok(c) => c,
            Err(e) => return Err(InterpreterError::CompileError(e.with_source_code(src))),
        };

        for (i, op) in chunk.code.iter().enumerate() {
            debug!("{}", chunk.disassemble_at(&src, i));
            debug!("          {}", self.trace_stack());
            match op {
                Op::Return => {
                    let res = self.pop();
                    println!("{}", res);
                    return Ok(Some(res));
                }
                Op::Constant(index) => {
                    let constant = chunk.constants[*index as usize];
                    self.push(constant);
                }
                Op::Negate => {
                    let old = self.pop();
                    self.push(-old)
                }
                Op::Add => binary_operator!(self, +),
                Op::Subtract => binary_operator!(self, -),
                Op::Multiply => binary_operator!(self, *),
                Op::Divide => binary_operator!(self, /),
            }
        }
        Err(InterpreterError::RuntimeError(miette::miette! {
            "Unexpected end of bytecode"
        }))
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
