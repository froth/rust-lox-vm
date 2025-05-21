use std::fmt::Display;

use super::function::Function;
use super::obj_ref::ObjRef;
use super::string::LoxString;
use super::value::Value;
use super::Hashable;
use crate::types::Hash;
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
    Native(fn(u8, *mut Value) -> Value),
    Closure {
        function: ObjRef,
        upvalues: Vec<ObjRef>,
    },
    Upvalue {
        location: *mut Value,
        next: Option<ObjRef>,
        closed: Value,
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
        }
    }
}
