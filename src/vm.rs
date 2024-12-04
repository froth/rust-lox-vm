use std::{fmt::Write as _, ops::Deref};

use miette::{LabeledSpan, NamedSource};
use tracing::debug;

use crate::{
    chunk::Chunk,
    compiler::Compiler,
    error::InterpreterError,
    gc::Gc,
    op::Op,
    types::{obj::Obj, value::Value},
};

const STACK_SIZE: usize = 256;

pub struct VM {
    stack: Box<[Value; STACK_SIZE]>,
    stack_top: *mut Value,
    gc: Gc,
}

macro_rules! binary_operator {
    ($self: ident, $chunk: ident, $i: ident, $op:tt, $constructor: expr) => {
        {
            if let (Value::Number(a), Value::Number(b)) = ($self.peek(1), $self.peek(0)) {
                let _ = $self.pop();
                let _ = $self.pop();
                $self.push($constructor(a $op b));
            } else {
                miette::bail!(
                    labels = vec![LabeledSpan::at($chunk.locations[$i], "here")],
                    "Operands for operation must be both be numbers"
                );
            }
        }
    };
}

impl VM {
    pub fn new() -> Self {
        let mut stack = Box::new([Value::Nil; STACK_SIZE]);
        let stack_top = stack.as_mut_ptr();
        let gc = Gc::new();
        Self {
            stack,
            stack_top,
            gc,
        }
    }

    pub fn interpret(
        &mut self,
        src: NamedSource<String>,
    ) -> std::result::Result<(), InterpreterError> {
        let chunk = match Compiler::compile(&src, &mut self.gc) {
            Ok(c) => c,
            Err(e) => return Err(InterpreterError::CompileError(e.with_source_code(src))),
        };

        match self.interpret_inner(&src, chunk) {
            Ok(value) => Ok(value),
            Err(e) => {
                self.reset_stack();
                Err(InterpreterError::RuntimeError(e.with_source_code(src)))
            }
        }
    }

    fn interpret_inner(&mut self, src: &NamedSource<String>, chunk: Chunk) -> miette::Result<()> {
        for (i, op) in chunk.code.iter().enumerate() {
            debug!("{}", chunk.disassemble_at(src, i));
            debug!("          {}", self.trace_stack());
            match op {
                Op::Return => {
                    todo!()
                }
                Op::Constant(index) => {
                    let constant = chunk.constants[*index as usize];
                    self.push(constant);
                }
                Op::Nil => self.push(Value::Nil),
                Op::True => self.push(Value::Boolean(true)),
                Op::False => self.push(Value::Boolean(false)),
                Op::Negate => {
                    let peek = self.peek(0);
                    if let Value::Number(number) = peek {
                        let _ = self.pop();
                        self.push(Value::Number(-number));
                    } else {
                        miette::bail!(
                            labels = vec![LabeledSpan::at(chunk.locations[i], "here")],
                            "Operand for unary - must be a number"
                        );
                    }
                }
                Op::Add => self.plus_operator(&chunk, i)?,
                Op::Subtract => binary_operator!(self, chunk,i,  -, Value::Number),
                Op::Multiply => binary_operator!(self, chunk,i, *, Value::Number),
                Op::Divide => binary_operator!(self, chunk,i, /, Value::Number),
                Op::Not => {
                    let pop = self.pop();
                    self.push(Value::Boolean(pop.is_falsey()))
                }
                Op::Equal => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(Value::Boolean(a == b));
                }
                Op::Greater => binary_operator!(self, chunk,i, >, Value::Boolean),
                Op::Less => binary_operator!(self, chunk,i, <, Value::Boolean),
                Op::Print => {
                    let res = self.pop();
                    println!("{}", res);
                    return Ok(());
                }
            }
        }
        Err(miette::miette! {
            "Unexpected end of bytecode"
        })
    }

    fn plus_operator(&mut self, chunk: &Chunk, index: usize) -> miette::Result<()> {
        match (self.peek(1), self.peek(0)) {
            (Value::Number(a), Value::Number(b)) => {
                self.pop();
                self.pop();
                self.push(Value::Number(a + b));
            }
            (Value::Obj(a), Value::Obj(b)) => {
                if let (Obj::String(a), Obj::String(b)) = (a.deref(), b.deref()) {
                    self.pop();
                    self.pop();
                    let concated = self.gc.manage_string(a.string.to_owned() + &b.string);
                    self.push(Value::Obj(concated));
                } else {
                    miette::bail!(
                        labels = vec![LabeledSpan::at(chunk.locations[index], "here")],
                        "Operands for operation must be both be numbers or Strings"
                    )
                }
            }
            _ => miette::bail!(
                labels = vec![LabeledSpan::at(chunk.locations[index], "here")],
                "Operands for operation must be both be numbers or Strings"
            ),
        }
        Ok(())
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

    fn peek(&self, distance: usize) -> Value {
        // SAFETY: NOT SAFE, stack could overflow and underflow
        unsafe { *self.stack_top.sub(1 + distance) }
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

    fn reset_stack(&mut self) {
        self.stack_top = self.stack.as_mut_ptr();
    }
}
