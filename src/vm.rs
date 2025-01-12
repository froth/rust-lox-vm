use std::{fmt::Write as _, ops::Deref, ptr::null};

use miette::{LabeledSpan, NamedSource, SourceSpan};
use tracing::debug;

use crate::{
    chunk::Chunk,
    datastructures::hash_table::HashTable,
    error::InterpreterError,
    gc::Gc,
    op::Op,
    parser::Parser,
    printer::{ConsolePrinter, Printer},
    types::{obj::Obj, value::Value},
};

const STACK_SIZE: usize = 256;

pub struct VM {
    stack: Box<[Value; STACK_SIZE]>,
    stack_top: *mut Value,
    gc: Gc,
    globals: HashTable,
    printer: Box<dyn Printer>,
    current: Chunk,
    ip: *const Op,
}

macro_rules! binary_operator {
    ($self: ident, $op:tt, $constructor: expr) => {
        {
            if let (Value::Number(a), Value::Number(b)) = ($self.peek(1), $self.peek(0)) {
                let _ = $self.pop();
                let _ = $self.pop();
                $self.push($constructor(a $op b));
            } else {
                miette::bail!(
                    labels = vec![LabeledSpan::at($self.current_location(), "here")],
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
        let globals = HashTable::new();
        let chunk = Chunk::new();
        let ip: *const Op = null();
        Self {
            stack,
            stack_top,
            gc,
            globals,
            printer: Box::new(ConsolePrinter),
            current: chunk,
            ip,
        }
    }

    pub fn interpret(
        &mut self,
        src: NamedSource<String>,
    ) -> std::result::Result<(), InterpreterError> {
        let chunk = match Parser::compile(&src, &mut self.gc) {
            Ok(c) => c,
            Err(e) => return Err(InterpreterError::CompileError(e.with_source_code(src))),
        };

        self.set_chunk(chunk);

        match self.interpret_inner(&src) {
            Ok(value) => Ok(value),
            Err(e) => {
                self.reset_stack();
                Err(InterpreterError::RuntimeError(e.with_source_code(src)))
            }
        }
    }

    fn set_chunk(&mut self, chunk: Chunk) {
        self.ip = &chunk.code[0];
        self.current = chunk;
    }

    fn current_index(&self) -> usize {
        unsafe { self.ip.offset_from(&self.current.code[0]) as usize }
    }

    fn current_location(&self) -> SourceSpan {
        self.current.locations[self.current_index()]
    }

    fn interpret_inner(&mut self, src: &NamedSource<String>) -> miette::Result<()> {
        loop {
            let op = unsafe { *self.ip };
            debug!("{}", self.current.disassemble_at(src, self.current_index()));
            debug!("          {}", self.trace_stack());
            unsafe {
                self.ip = self.ip.add(1);
            }
            match op {
                Op::Return => return Ok(()),
                Op::Constant(index) => {
                    let constant = self.current.constants[index as usize];
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
                            labels = vec![LabeledSpan::at(self.current_location(), "here")],
                            "Operand for unary - must be a number"
                        );
                    }
                }
                Op::Add => self.plus_operator()?,
                Op::Subtract => binary_operator!(self, -, Value::Number),
                Op::Multiply => binary_operator!(self, *, Value::Number),
                Op::Divide => binary_operator!(self, /, Value::Number),
                Op::Not => {
                    let pop = self.pop();
                    self.push(Value::Boolean(pop.is_falsey()))
                }
                Op::Equal => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(Value::Boolean(a == b));
                }
                Op::Greater => binary_operator!(self, >, Value::Boolean),
                Op::Less => binary_operator!(self, <, Value::Boolean),
                Op::Print => {
                    let res = self.pop();
                    self.printer.print(res);
                }
                Op::Pop => {
                    self.pop();
                }
                Op::DefineGlobal(index) => {
                    let name = self.current.constants[index as usize];
                    self.globals.insert(name, self.peek(0));
                    self.pop();
                }
                Op::GetGlobal(index) => {
                    let name = self.current.constants[index as usize];
                    if let Some(v) = self.globals.get(name) {
                        self.push(v)
                    } else {
                        miette::bail!(
                            labels = vec![LabeledSpan::at(self.current_location(), "here")],
                            "Undefined variable {}",
                            name
                        )
                    }
                }
                Op::SetGlobal(index) => {
                    let name = self.current.constants[index as usize];
                    let inserted = self.globals.insert(name, self.peek(0));
                    if inserted {
                        self.globals.delete(name);
                        miette::bail!(
                            labels = vec![LabeledSpan::at(self.current_location(), "here")],
                            "Undefined variable {}",
                            name
                        )
                    }
                }
                Op::GetLocal(slot) => {
                    self.push(self.stack[slot as usize]);
                }
                Op::SetLocal(slot) => self.stack[slot as usize] = self.peek(0),
                Op::JumpIfFalse(offset) => {
                    if self.peek(0).is_falsey() {
                        unsafe { self.ip = self.ip.add((offset - 1) as usize) }
                    }
                }
                Op::Jump(offset) => unsafe {
                    self.ip = self.ip.add((offset - 1) as usize);
                },
                Op::Loop(offset) => unsafe {
                    self.ip = self.ip.sub((offset + 1) as usize);
                },
            }
        }
    }

    fn plus_operator(&mut self) -> miette::Result<()> {
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
                        labels = vec![LabeledSpan::at(self.current_location(), "here")],
                        "Operands for operation must be both be numbers or Strings"
                    )
                }
            }
            _ => miette::bail!(
                labels = vec![LabeledSpan::at(self.current_location(), "here")],
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

#[cfg(test)]
mod tests {
    use datadriven::walk;
    use miette::NamedSource;
    use serde_json::Value;

    use crate::printer::{vec_printer::VecPrinter, Printer};

    use super::VM;

    impl VM {
        pub fn with_printer(printer: Box<dyn Printer>) -> Self {
            let mut vm = VM::new();
            vm.printer = printer;
            vm
        }
    }
    #[test]
    fn integration_tests() {
        walk("tests/", |f| {
            let file_name = f.filename.clone();
            f.run(|test_case| -> String {
                let input = test_case.input.to_string();
                let printer = VecPrinter::new();
                let mut vm = VM::with_printer(Box::new(printer.clone()));
                let named_source = NamedSource::new(file_name.clone(), input.clone());
                let result = vm.interpret(named_source);
                assert_eq!(vm.stack_top, vm.stack.as_mut_ptr(), "Stack is not empty");
                if test_case.directive == "error" {
                    let err = result.expect_err(
                        format!("Test {file_name} meant to be failing but succeeded").as_str(),
                    );
                    let handler = miette::JSONReportHandler::new();
                    let mut json = String::new();
                    handler.render_report(&mut json, &err).unwrap();
                    format_json(json)
                } else {
                    result.unwrap_or_else(|_| {
                        panic!("Test {file_name} meant to be succeeding but failed.")
                    });
                    printer.get_output()
                }
            })
        });
    }

    fn format_json(json: String) -> String {
        let x: Value = serde_json::from_str(json.as_str()).unwrap();
        serde_json::to_string_pretty(&x).unwrap()
    }
}
