mod callframe;
mod gc;
mod native_functions;

use std::{
    alloc::{self, Layout},
    fmt::Write as _,
    ops::{Deref, DerefMut},
};

use callframe::CallFrame;
use miette::{LabeledSpan, NamedSource};
use tracing::debug;

use crate::{
    datastructures::hash_table::HashTable,
    error::InterpreterError,
    gc::Gc,
    op::Op,
    parser::Parser,
    printer::{ConsolePrinter, Printer},
    types::{
        bound_method::BoundMethod, class::Class, closure::Closure, instance::Instance, obj::Obj,
        obj_ref::ObjRef, value::Value,
    },
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
    open_upvalues: Option<ObjRef>,
    init_string: Value,
}

struct UpvalueLocation {
    location: *mut Value,
    current: ObjRef,
    next: Option<ObjRef>,
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
        let mut gc = Gc::new();
        let init_string = Value::Obj(gc.alloc("init"));
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
            open_upvalues: None,
            init_string,
        };
        vm.define_native_functions();
        vm
    }

    pub fn heapdump(&self) {
        self.gc.heapdump()
    }

    pub fn interpret(
        &mut self,
        src: NamedSource<String>,
    ) -> std::result::Result<(), InterpreterError> {
        let function = match Parser::compile(&src, &mut self.gc) {
            Ok(c) => c,
            Err(e) => return Err(InterpreterError::CompileError(e.with_source_code(src))),
        };

        let function = self.gc.alloc(function); // gc.alloc to prevent collection
        self.push(Value::Obj(function));
        let closure = self.alloc(Obj::Closure(Closure::new(function, vec![])));
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
                    self.close_upvalues(slots);
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
                    let pop: Value = self.pop();
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
                    if let Obj::Upvalue { location, .. } = upvalue.deref() {
                        self.push(**location);
                    } else {
                        unreachable!()
                    }
                },
                Op::SetUpvalue(index) => unsafe {
                    let upvalue = self.current_frame().upvalues()[index as usize];
                    if let Obj::Upvalue { location, .. } = upvalue.deref() {
                        **location = self.peek(0)
                    } else {
                        unreachable!()
                    }
                },
                Op::GetProperty(index) => self.get_property(index)?,
                Op::SetProperty(index) => self.set_property(index)?,
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
                    self.handle_closure(index);
                }
                Op::CloseUpvalue => unsafe {
                    self.close_upvalues(self.stack_top.sub(1));
                    self.pop();
                },
                Op::Class(index) => self.create_class(index),
                Op::Method(index) => self.define_method(index),
                Op::Invoke {
                    property_index,
                    arg_count,
                } => {
                    let constant = self.current_frame().chunk().constants[property_index as usize];
                    self.invoke(constant, arg_count)?
                }
                Op::SuperInvoke {
                    property_index,
                    arg_count,
                } => {
                    let constant = self.current_frame().chunk().constants[property_index as usize];
                    self.super_invoke(constant, arg_count)?
                }
                Op::Inherit => {
                    let superclass = self.peek(1);
                    if let Value::Obj(obj) = superclass {
                        if let Obj::Class(superclass) = obj.deref() {
                            let mut peek = self.peek(0);
                            let subclass = peek.as_class_mut();
                            subclass.copy_methods(superclass);
                            self.pop(); // pop subclass
                            continue;
                        }
                    }
                    miette::bail!(
                        labels = vec![LabeledSpan::at(
                            self.current_frame().current_location(),
                            "here"
                        )],
                        "Superclass must be a class, not {}",
                        superclass
                    )
                }
                Op::GetSuper(index) => {
                    let name = self.current_frame().chunk().constants[index as usize];
                    let superclass = self.pop();
                    let superclass = superclass.as_class();
                    self.bind_method(superclass, name)?;
                }
            }
        }
    }

    fn define_method(&mut self, index: u8) {
        let name = self.current_frame().chunk().constants[index as usize];
        let method = self.peek(0);
        let mut peek = self.peek(1);
        peek.as_class_mut().add_method(name, method);
        self.pop();
    }

    fn super_invoke(&mut self, name: Value, arg_count: u8) -> Result<(), miette::Error> {
        let super_class = self.pop();
        let super_class = super_class.as_class();
        self.invoke_from_class(super_class, name, arg_count)
    }
    fn invoke(&mut self, name: Value, arg_count: u8) -> Result<(), miette::Error> {
        let receiver = self.peek(arg_count);
        if let Value::Obj(obj) = receiver {
            if let Obj::Instance(instance) = obj.deref() {
                if let Some(value) = instance.get_field(name) {
                    unsafe { *(self.stack_top.sub(arg_count as usize).sub(1)) = value };
                    self.call_value(value, arg_count)
                } else {
                    self.invoke_from_class(instance.class(), name, arg_count)
                }
            } else {
                miette::bail!(
                    labels = vec![LabeledSpan::at(
                        self.current_frame().current_location(),
                        "here"
                    )],
                    "Can only invoke methods on instances, not on {}",
                    receiver
                )
            }
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(
                    self.current_frame().current_location(),
                    "here"
                )],
                "Can only invoke methods on instances, not on {}",
                receiver
            )
        }
    }

    fn invoke_from_class(
        &mut self,
        class: &Class,
        name: Value,
        arg_count: u8,
    ) -> Result<(), miette::Error> {
        let method = if let Some(method) = class.get_method(name) {
            method
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(
                    self.current_frame().current_location(),
                    "here"
                )],
                "Undefined property {} on class {}",
                name,
                class.name()
            )
        };
        self.call(arg_count, *method.as_obj(), method.as_closure())
    }

    fn get_property(&mut self, index: u8) -> Result<(), miette::Error> {
        let name = self.current_frame().chunk().constants[index as usize];
        let obj = if let Value::Obj(obj) = self.peek(0) {
            obj
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(
                    self.current_frame().current_location(),
                    "here"
                )],
                "Can only get properties on objects, not on {}",
                self.peek(0)
            )
        };
        let instance = if let Obj::Instance(instance) = obj.deref() {
            instance
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(
                    self.current_frame().current_location(),
                    "here"
                )],
                "Can only get properties on instances, not on {}",
                obj
            )
        };

        if let Some(field) = instance.get_field(name) {
            self.pop();
            self.push(field);
            Ok(())
        } else {
            self.bind_method(instance.class(), name)
        }
    }

    fn set_property(&mut self, index: u8) -> Result<(), miette::Error> {
        let name = self.current_frame().chunk().constants[index as usize];
        let mut obj = if let Value::Obj(obj) = self.peek(1) {
            obj
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(
                    self.current_frame().current_location(),
                    "here"
                )],
                "Can only get properties on objects, not on {}",
                self.peek(0)
            )
        };
        if let Obj::Instance(instance) = obj.deref_mut() {
            instance.set_field(name, self.peek(0));
            let value = self.pop();
            self.pop();
            self.push(value);
            Ok(())
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(
                    self.current_frame().current_location(),
                    "here"
                )],
                "Can only get properties on instances, not on {}",
                self.peek(0)
            )
        }
    }

    fn bind_method(&mut self, class: &Class, name: Value) -> Result<(), miette::Error> {
        let method = if let Some(method) = class.get_method(name) {
            method
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(
                    self.current_frame().current_location(),
                    "here"
                )],
                "Undefined property {} on class {}",
                name,
                class.name()
            )
        };

        let bound_method = BoundMethod::new(self.peek(0), *method.as_obj());
        let bound_method = self.gc.alloc(Obj::BoundMethod(bound_method));
        self.pop();
        self.push(Value::Obj(bound_method));
        Ok(())
    }

    fn create_class(&mut self, index: u8) {
        let name = self.current_frame().chunk().constants[index as usize];
        let name = name.as_string();
        let class = Obj::Class(Class::new(name.clone()));
        let class = self.gc.alloc(class);
        self.push(Value::Obj(class));
    }

    fn close_upvalues(&mut self, last: *mut Value) {
        while let Some(UpvalueLocation {
            location,
            mut current,
            next: _,
        }) = Self::upvalue_location(self.open_upvalues)
        {
            if location < last {
                break;
            }
            if let Obj::Upvalue {
                location,
                next,
                closed,
            } = current.deref_mut()
            {
                unsafe { *closed = **location };
                *location = closed;
                self.open_upvalues = *next;
            } else {
                unreachable!()
            }
        }
    }

    fn handle_closure(&mut self, index: u8) {
        let obj = self.current_frame().chunk().constants[index as usize];
        let function = obj.as_function();
        let upvalues = function
            .upvalues()
            .iter()
            .map(|u| {
                if u.is_local() {
                    let value = unsafe { self.current_frame().slots.add(u.index() as usize) };
                    self.capture_upvalue(value)
                } else {
                    self.current_frame().upvalues()[u.index() as usize]
                }
            })
            .collect();
        let closure = Obj::Closure(Closure::new(*obj.as_obj(), upvalues));
        let closure = self.alloc(closure);
        self.push(Value::Obj(closure));
    }

    fn capture_upvalue(&mut self, local: *mut Value) -> ObjRef {
        let mut prev_upvalue: Option<ObjRef> = None;
        let mut upvalue = self.open_upvalues;
        while let Some(UpvalueLocation {
            location,
            current,
            next,
        }) = Self::upvalue_location(upvalue)
        {
            if location <= local {
                break;
            }
            prev_upvalue = Some(current);
            upvalue = next;
        }

        if let Some(UpvalueLocation {
            location,
            current,
            next: _,
        }) = Self::upvalue_location(upvalue)
        {
            if location == local {
                return current;
            }
        }

        let created_upvalue = Obj::Upvalue {
            location: local,
            next: upvalue,
            closed: Value::Nil,
        };
        let created_upvalue = self.alloc(created_upvalue);
        if let Some(mut obj) = prev_upvalue {
            if let Obj::Upvalue { next, .. } = obj.deref_mut() {
                *next = Some(created_upvalue);
            } else {
                unreachable!()
            }
        } else {
            self.open_upvalues = Some(created_upvalue);
        }
        created_upvalue
    }

    fn upvalue_location(upvalue: Option<ObjRef>) -> Option<UpvalueLocation> {
        if let Some(Obj::Upvalue { location, next, .. }) = upvalue.as_deref() {
            Some(UpvalueLocation {
                location: *location,
                current: upvalue.unwrap(),
                next: *next,
            })
        } else {
            None
        }
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
        let obj = if let Value::Obj(obj) = callee {
            obj
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(
                    self.current_frame().current_location(),
                    "here"
                )],
                "Can only call functions or classes.",
            )
        };
        match obj.deref() {
            Obj::Closure(closure) => self.call(arg_count, obj, closure),
            Obj::Native(function) => unsafe {
                let result = function(arg_count, self.stack_top.sub(arg_count as usize), self);
                self.stack_top = self.stack_top.sub(arg_count as usize + 1);
                self.push(result);
                Ok(())
            },
            Obj::Class(class) => unsafe {
                let instance = Obj::Instance(Instance::new(obj));
                let instance = self.gc.alloc(instance);
                *self.stack_top.sub(arg_count as usize).sub(1) = Value::Obj(instance);
                if let Some(initializer) = class.get_method(self.init_string) {
                    self.call(arg_count, *initializer.as_obj(), initializer.as_closure())?;
                } else if arg_count > 0 {
                    miette::bail!(
                        labels = vec![LabeledSpan::at(
                            self.current_frame().current_location(),
                            "here"
                        )],
                        "Expected 0 arguments but got {}.",
                        arg_count
                    );
                }
                Ok(())
            },
            Obj::BoundMethod(bound_method) => {
                unsafe {
                    *self.stack_top.sub(arg_count as usize).sub(1) = bound_method.receiver();
                }
                self.call(
                    arg_count,
                    bound_method.method(),
                    bound_method.method().as_closure(),
                )
            }
            _ => miette::bail!(
                labels = vec![LabeledSpan::at(
                    self.current_frame().current_location(),
                    "here"
                )],
                "Can only call closures or classes.",
            ),
        }
    }

    fn call(&mut self, arg_count: u8, obj: ObjRef, closure: &Closure) -> miette::Result<()> {
        let function = closure.function.as_function();
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
                    let concated = self.alloc(a.string.to_owned() + &b.string);
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
    fn miri_test_for() {
        let input = "for (var a = 1; a<5; a = a +1) {print a;}".to_string();
        let printer = VecPrinter::new();
        let mut vm = VM::with_printer(Box::new(printer.clone()));
        let named_source = NamedSource::new("miri_test", input.clone());
        vm.interpret(named_source).unwrap();
        assert_eq!(vm.stack_top, vm.stack, "Stack is not empty");
        assert_eq!(printer.get_output(), "1\n2\n3\n4\n");
    }

    #[test]
    fn miri_test_closure() {
        let input = r#"
fun outer() {
  var x = "outside";
  fun inner() {
    var y = 1;
    print x;
  }
  return inner;
}

var closure = outer();
closure();"#
            .to_string();
        let printer = VecPrinter::new();
        let mut vm = VM::with_printer(Box::new(printer.clone()));
        let named_source = NamedSource::new("miri_test", input.clone());
        vm.interpret(named_source).unwrap();
        assert_eq!(vm.stack_top, vm.stack, "Stack is not empty");
        assert_eq!(printer.get_output(), "outside\n");
    }

    fn format_json(json: String) -> String {
        let x: Value = serde_json::from_str(json.as_str()).unwrap();
        serde_json::to_string_pretty(&x).unwrap()
    }
}
