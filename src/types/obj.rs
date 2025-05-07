use std::fmt::Display;

use super::function::Function;
use super::obj_ref::ObjRef;
use super::string::LoxString;
use super::value::Value;
use super::Hashable;
use crate::types::Hash;

pub enum Obj {
    String(LoxString),
    Function(Function),
    Native(fn(u8, *mut Value) -> Value),
    Closure { function: ObjRef },
}

impl Hashable for Obj {
    fn hash(&self) -> super::Hash {
        match self {
            Obj::String(lox_string) => lox_string.hash(),
            Obj::Function(function) => function.hash(),
            Obj::Native(_) => Hash(11),
            Obj::Closure { function } => function.hash(),
        }
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Obj::String(s) => write!(f, "{}", s.string),
            Obj::Function(function) => write!(f, "{}", function),
            Obj::Native(_) => write!(f, "<native fn>"),
            Obj::Closure { function } => write!(f, "{}", function),
        }
    }
}
