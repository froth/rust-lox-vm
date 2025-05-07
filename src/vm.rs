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
    types::{function::Function, obj::Obj, obj_ref::ObjRef, value::Value},
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
    sources: Vec<NamedSource<String>>,
}

struct CallFrame {
    closure: ObjRef,
    ip: *const Op,
    slots: *mut Value,
}

impl CallFrame {
    fn function(&self) -> &Function {
        if let Obj::Closure {
            function,
            upvalues: _,
        } = self.closure.deref()
        {
            if let Obj::Function(function) = function.deref() {
                return function;
            }
        }
        unreachable!("callframe stored non-closure")
    }

    fn upvalues(&self) -> &[ObjRef] {
        if let Obj::Closure {
            function: _,
            upvalues,
        } = self.closure.deref()
        {
            upvalues
        } else {
            unreachable!("callframe stored non-closure")
        }
    }

    fn chunk(&self) -> &Chunk {
        self.function().chunk()
    }
    fn current_index(&self) -> usize {
        unsafe { self.ip.offset_from(&(*self.function()).chunk().code[0]) as usize }
    }

    fn current_location(&self) -> SourceSpan {
        self.chunk().locations[self.current_index()]
    }

    fn disassemble_at_current_index(&mut self) -> String {
        let current_index = self.current_index();
        self.chunk().disassemble_at(current_index)
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
        let mut vm = Self {
            stack,
            stack_top: stack,
            frames,
            frame_count: 0,
            gc,
            globals,
            printer: Box::new(ConsolePrinter),
            sources: vec![],
        };
        vm.define_native("clock", |_, _| {
            let millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();
            Value::Number(millis)
        });
        vm
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
        let closure = self.gc.manage(Obj::Closure {
            function,
            upvalues: vec![],
        });
        self.pop();
        self.push(Value::Obj(closure));

        let arg_count = 0;
        self.call_value(self.peek(arg_count), 0)
            .map_err(|e| InterpreterError::RuntimeError {
                error: e.with_source_code(src.clone()),
                stacktrace: self.stacktrace(),
            })?;

        self.sources.push(src);

        match self.interpret_inner() {
            Ok(value) => Ok(value),
            Err(e) => {
                let stacktrace = self.stacktrace();
                self.reset_stack();
                let error = e.with_source_code(self.current_frame().chunk().source.clone());
                Err(InterpreterError::RuntimeError { error, stacktrace })
            }
        }
    }

    fn interpret_inner(&mut self) -> miette::Result<()> {
        loop {
            let op = unsafe { *ip!(self) };
            debug!("{}", self.current_frame().disassemble_at_current_index());
            debug!("          {}", self.trace_stack());
            unsafe {
                ip!(self) = ip!(self).add(1);
            }
            match op {
                Op::Return => {
                    let result = self.pop();
                    let slots = self.current_frame().slots;
                    self.frame_count -= 1;
                    if self.frame_count == 0 {
                        self.pop();
                        return Ok(());
                    }
                    self.stack_top = slots;
                    self.push(result);
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
                Op::GetUpvalue(index) => unsafe {
                    let upvalue = self.current_frame().upvalues()[index as usize];
                    if let Obj::Upvalue(location) = upvalue.deref() {
                        self.push(**location);
                    } else {
                        unreachable!()
                    }
                },
                Op::SetUpvalue(index) => unsafe {
                    let upvalue = self.current_frame().upvalues()[index as usize];
                    if let Obj::Upvalue(location) = upvalue.deref() {
                        **location = self.peek(0)
                    } else {
                        unreachable!()
                    }
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
                Op::Call(arg_count) => {
                    let callee = self.peek(arg_count);
                    self.call_value(callee, arg_count)?
                }
                Op::Closure(index) => {
                    let function = self.current_frame().chunk().constants[index as usize];
                    if let Value::Obj(obj) = function {
                        if let Obj::Function(f) = obj.deref() {
                            let upvalues = f
                                .upvalues()
                                .iter()
                                .map(|u| {
                                    if u.is_local() {
                                        let value = unsafe {
                                            self.current_frame().slots.add(index as usize)
                                        };
                                        self.capture_upvalue(value)
                                    } else {
                                        self.current_frame().upvalues()[index as usize]
                                    }
                                })
                                .collect();
                            let closure = Obj::Closure {
                                function: obj,
                                upvalues,
                            };
                            let closure = self.gc.manage(closure);
                            self.push(Value::Obj(closure));
                        } else {
                            unreachable!(
                                "expected function at closure index but was {:?}",
                                function
                            );
                        }
                    } else {
                        unreachable!("expected function at closure index but was {:?}", function);
                    }
                }
            }
        }
    }

    fn capture_upvalue(&mut self, local: *mut Value) -> ObjRef {
        let upvalue = Obj::Upvalue(local);
        self.gc.manage(upvalue)
    }

    fn current_frame(&mut self) -> &mut CallFrame {
        unsafe { &mut *self.frames.add(self.frame_count - 1) }
    }

    fn call_value(&mut self, callee: Value, arg_count: u8) -> miette::Result<()> {
        if self.frame_count == FRAMES_MAX {
            miette::bail!(
                labels = vec![LabeledSpan::at(
                    self.current_frame().current_location(),
                    "here"
                )],
                "Stack overflow",
            )
        }
        if let Value::Obj(obj) = callee {
            match obj.deref() {
                Obj::Closure {
                    function,
                    upvalues: _,
                } => {
                    if let Obj::Function(function) = function.deref() {
                        if function.arity() != arg_count {
                            miette::bail!(
                                labels = vec![LabeledSpan::at(
                                    self.current_frame().current_location(),
                                    "here"
                                )],
                                "Expected {} arguments but got {}",
                                function.arity(),
                                arg_count
                            )
                        }
                        unsafe {
                            let frame = self.frames.add(self.frame_count);
                            (*frame).closure = obj;
                            (*frame).ip = function.chunk().code.ptr();
                            (*frame).slots = self.stack_top.sub(arg_count as usize + 1);
                        }
                        self.frame_count += 1;
                        Ok(())
                    } else {
                        unreachable!("non function wrapped in closure")
                    }
                }
                Obj::Native(function) => unsafe {
                    let result = function(arg_count, self.stack_top.sub(arg_count as usize));
                    self.stack_top = self.stack_top.sub(arg_count as usize + 1);
                    self.push(result);
                    Ok(())
                },
                _ => miette::bail!(
                    labels = vec![LabeledSpan::at(
                        self.current_frame().current_location(),
                        "here"
                    )],
                    "Can only call closures or classes.",
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

    fn peek(&self, distance: u8) -> Value {
        // SAFETY: NOT SAFE, stack could overflow and underflow
        unsafe { *self.stack_top.sub(1 + distance as usize) }
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

    fn stacktrace(&self) -> String {
        let mut trace = String::new();
        for i in (0..(self.frame_count)).rev() {
            unsafe {
                let frame = self.frames.add(i);
                let instruction = (*frame).ip.offset_from((*frame).chunk().code.ptr()) - 1;
                let line = (*frame).chunk().line_number(instruction as usize);
                let _ = write!(trace, "[line {}] in ", line);
                if let Some(name) = (*(*frame).function()).name() {
                    let _ = writeln!(trace, "{}()", name.string);
                } else {
                    let _ = writeln!(trace, "script");
                }
            }
        }
        trace
    }

    fn define_native(&mut self, name: &str, function: fn(u8, *mut Value) -> Value) {
        let name = self.gc.manage_str(name);
        self.push(Value::Obj(name));
        let function = self.gc.manage(Obj::Native(function));
        self.push(Value::Obj(function));
        self.globals.insert(self.peek(1), self.peek(0));
        self.pop();
        self.pop();
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
