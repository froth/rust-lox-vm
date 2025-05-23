use std::fmt::{Debug, Display};
use std::ops::Deref;

use super::function::Function;
use super::obj_ref::ObjRef;
use super::string::LoxString;
use super::value::Value;
use super::Hashable;
use crate::types::Hash;
use crate::vm::VM;
pub struct ObjStruct {
    pub obj: Obj,
    pub marked: bool,
}

impl ObjStruct {
    pub fn new(obj: Obj) -> Self {
        Self { obj, marked: false }
    }
}
pub enum Obj {
    String(LoxString),
    Function(Function),
    Native(fn(u8, *mut Value, &VM) -> Value),
    Closure {
        function: ObjRef,
        upvalues: Vec<ObjRef>,
    },
    Upvalue {
        location: *mut Value,
        next: Option<ObjRef>,
        closed: Value,
    },
    Class {
        name: LoxString,
    },
}

impl Hashable for Obj {
    fn hash(&self) -> super::Hash {
        match self {
            Obj::String(lox_string) => lox_string.hash(),
            Obj::Function(function) => function.hash(),
            Obj::Native(_) => Hash(11),
            Obj::Closure {
                function,
                upvalues: _,
            } => function.hash(),
            Obj::Upvalue {
                location: value, ..
            } => unsafe { (**value).hash() },
            Obj::Class { name } => name.hash(),
        }
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Obj::String(s) => write!(f, "{}", s.string),
            Obj::Function(function) => write!(f, "{}", function),
            Obj::Native(_) => write!(f, "<native fn>"),
            Obj::Closure {
                function,
                upvalues: _,
            } => write!(f, "closure over {}", function),
            Obj::Upvalue { location: _, .. } => write!(f, "upvalue"),
            Obj::Class { name } => write!(f, "{}", name),
        }
    }
}

impl Debug for Obj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(arg0) => f.debug_tuple("String").field(arg0).finish(),
            Self::Function(arg0) => f.debug_tuple("Function").field(arg0).finish(),
            Self::Native(arg0) => f.debug_tuple("Native").field(arg0).finish(),
            Self::Closure { function, upvalues } => {
                let function_name = if let Self::Function(f) = function.deref() {
                    f.name().map(|n| n.string.clone())
                } else {
                    unreachable!()
                };
                f.debug_struct("Closure")
                    .field("function", &function_name)
                    .field("upvalues", upvalues)
                    .finish()
            }
            Self::Upvalue {
                location,
                next,
                closed,
            } => {
                let closed_ptr: *const Value = closed;
                let is_closed = location.addr() == closed_ptr.addr();
                f.debug_struct("Upvalue")
                    .field("value", unsafe { &**location })
                    .field("closed", &is_closed)
                    .finish()
            }
            Self::Class { name } => f.debug_struct("Class").field("name", name).finish(),
        }
    }
}
