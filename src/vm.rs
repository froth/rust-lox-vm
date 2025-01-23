use std::{
    alloc::{self, Layout},
    fmt::Write as _,
    ops::Deref,
};

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
    types::{function::Function, obj::Obj, value::Value},
};

const FRAMES_MAX: usize = 64;
const STACK_MAX: usize = 256 * FRAMES_MAX; // us::

pub struct VM {
    stack: *mut Value,
    stack_top: *mut Value,
    frames: *mut CallFrame,
    frame_count: usize,
    gc: Gc,
    globals: HashTable,
    printer: Box<dyn Printer>,
}

struct CallFrame {
    function: *const Function,
    ip: *const Op,
    slots: *mut Value,
}

impl CallFrame {
    fn chunk(&self) -> &Chunk {
        unsafe { (*self.function).chunk() }
    }
    fn current_index(&self) -> usize {
        unsafe { self.ip.offset_from(&(*self.function).chunk().code[0]) as usize }
    }

    fn current_location(&self) -> SourceSpan {
        self.chunk().locations[self.current_index()]
    }

    fn disassemble_at_current_index(&mut self, src: &NamedSource<String>) -> String {
        let current_index = self.current_index();
        self.chunk().disassemble_at(src, current_index)
    }
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
                    labels = vec![LabeledSpan::at($self.current_frame().current_location(), "here")],
                    "Operands for operation must be both be numbers"
                );
            }
        }
    };
}

macro_rules! ip {
    ($self: ident) => {
        (*$self.frames.add($self.frame_count - 1)).ip
    };
}

impl VM {
    pub fn new() -> Self {
        //Safety: Layouts are guaranteed to be nonzero sized.
        let stack =
            unsafe { alloc::alloc(Layout::array::<Value>(STACK_MAX).unwrap()) as *mut Value };
        let frames =
            unsafe { alloc::alloc(Layout::array::<Value>(FRAMES_MAX).unwrap()) as *mut CallFrame };
        let gc = Gc::new();
        let globals = HashTable::new();
        Self {
            stack,
            stack_top: stack,
            frames,
            frame_count: 0,
            gc,
            globals,
            printer: Box::new(ConsolePrinter),
        }
    }

    pub fn interpret(
        &mut self,
        src: NamedSource<String>,
    ) -> std::result::Result<(), InterpreterError> {
        let function = match Parser::compile(&src, &mut self.gc) {
            Ok(c) => c,
            Err(e) => return Err(InterpreterError::CompileError(e.with_source_code(src))),
        };

        let function = self.gc.manage(function);
        self.push(Value::Obj(function));
        let arg_count = 0;
        self.call_value(self.peek(arg_count), arg_count)
            .map_err(|e| InterpreterError::RuntimeError(e.with_source_code(src.clone())))?;

        match self.interpret_inner(&src) {
            Ok(value) => Ok(value),
            Err(e) => {
                self.reset_stack();
                Err(InterpreterError::RuntimeError(e.with_source_code(src)))
            }
        }
    }

    fn interpret_inner(&mut self, src: &NamedSource<String>) -> miette::Result<()> {
        loop {
            let op = unsafe { *ip!(self) };
            debug!("{}", self.current_frame().disassemble_at_current_index(src));
            debug!("          {}", self.trace_stack());
            unsafe {
                ip!(self) = ip!(self).add(1);
            }
            match op {
                Op::Return => {
                    self.frame_count -= 1;
                    if self.frame_count == 0 {
                        self.pop();
                        return Ok(());
                    }
                    todo!()
                }
                Op::Constant(index) => {
                    let constant = self.current_frame().chunk().constants[index as usize];
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
                            labels = vec![LabeledSpan::at(
                                self.current_frame().current_location(),
                                "here"
                            )],
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
                    let name = self.current_frame().chunk().constants[index as usize];
                    self.globals.insert(name, self.peek(0));
                    self.pop();
                }
                Op::GetGlobal(index) => {
                    let name = self.current_frame().chunk().constants[index as usize];
                    if let Some(v) = self.globals.get(name) {
                        self.push(v)
                    } else {
                        miette::bail!(
                            labels = vec![LabeledSpan::at(
                                self.current_frame().current_location(),
                                "here"
                            )],
                            "Undefined variable {}",
                            name
                        )
                    }
                }
                Op::SetGlobal(index) => {
                    let name = self.current_frame().chunk().constants[index as usize];
                    let inserted = self.globals.insert(name, self.peek(0));
                    if inserted {
                        self.globals.delete(name);
                        miette::bail!(
                            labels = vec![LabeledSpan::at(
                                self.current_frame().current_location(),
                                "here"
                            )],
                            "Undefined variable {}",
                            name
                        )
                    }
                }
                Op::GetLocal(slot) => unsafe {
                    let slots = self.current_frame().slots;
                    self.push(*(slots.add(slot as usize)));
                },
                Op::SetLocal(slot) => unsafe {
                    let slots = self.current_frame().slots;
                    *(slots.add(slot as usize)) = self.peek(0)
                },
                Op::JumpIfFalse(offset) => {
                    if self.peek(0).is_falsey() {
                        unsafe { ip!(self) = ip!(self).add((offset - 1) as usize) }
                    }
                }
                Op::Jump(offset) => unsafe {
                    ip!(self) = ip!(self).add((offset - 1) as usize);
                },
                Op::Loop(offset) => unsafe {
                    ip!(self) = ip!(self).sub((offset + 1) as usize);
                },
            }
        }
    }

    fn current_frame(&mut self) -> &mut CallFrame {
        unsafe { &mut *self.frames.add(self.frame_count - 1) }
    }

    fn call_value(&mut self, callee: Value, arg_count: usize) -> miette::Result<()> {
        unsafe {
            if let Value::Obj(obj) = callee {
                match obj.deref() {
                    Obj::Function(function) => {
                        let frame = self.frames.add(self.frame_count);
                        (*frame).function = function;
                        (*frame).ip = function.chunk().code.ptr();
                        (*frame).slots = self.stack_top.sub(arg_count + 1);
                        self.frame_count += 1;
                        Ok(())
                    }
                    _ => miette::bail!(
                        labels = vec![LabeledSpan::at(
                            self.current_frame().current_location(),
                            "here"
                        )],
                        "Can only call functions or classes.",
                    ),
                }
            } else {
                miette::bail!(
                    labels = vec![LabeledSpan::at(
                        self.current_frame().current_location(),
                        "here"
                    )],
                    "Can only call functions or classes.",
                )
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
                        labels = vec![LabeledSpan::at(
                            self.current_frame().current_location(),
                            "here"
                        )],
                        "Operands for operation must be both be numbers or Strings"
                    )
                }
            }
            _ => miette::bail!(
                labels = vec![LabeledSpan::at(
                    self.current_frame().current_location(),
                    "here"
                )],
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
        let mut current = self.stack;
        while current != self.stack_top {
            let value = unsafe { *current };
            write!(&mut res, "[ {} ]", value).unwrap();
            unsafe { current = current.add(1) };
        }
        res
    }

    fn reset_stack(&mut self) {
        self.stack_top = self.stack;
    }
}

impl Drop for VM {
    fn drop(&mut self) {
        //Safety: Layouts are same as in new.
        unsafe {
            alloc::dealloc(
                self.stack as *mut u8,
                Layout::array::<Value>(STACK_MAX).unwrap(),
            );
            alloc::dealloc(
                self.frames as *mut u8,
                Layout::array::<Value>(FRAMES_MAX).unwrap(),
            );
        }
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
                assert_eq!(vm.stack_top, vm.stack, "Stack is not empty");
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

    // miri can not run on integration tests, as they require fileaccess
    #[test]
    fn miri_test() {
        let input = "for (var a = 1; a<5; a = a +1) {print a;}".to_string();
        let printer = VecPrinter::new();
        let mut vm = VM::with_printer(Box::new(printer.clone()));
        let named_source = NamedSource::new("miri_test", input.clone());
        vm.interpret(named_source).unwrap();
        assert_eq!(vm.stack_top, vm.stack, "Stack is not empty");
        assert_eq!(printer.get_output(), "1\n2\n3\n4\n");
    }

    fn format_json(json: String) -> String {
        let x: Value = serde_json::from_str(json.as_str()).unwrap();
        serde_json::to_string_pretty(&x).unwrap()
    }
}
