use std::{fmt::Display, ptr::NonNull};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Number(f64),
    Boolean(bool),
    Nil,
    Obj(NonNull<Obj>),
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        matches!(self, Value::Nil | Value::Boolean(false))
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "Nil"),
            Value::Obj(obj) => {
                //SAFETY: managed by GC
                unsafe { write!(f, "{}", *obj.as_ptr()) }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Obj {
    String(LoxString),
    Class { name: LoxString },
}

impl Obj {
    pub fn from_str(s: &str) -> Self {
        Self::String(LoxString::string(s.to_owned()))
    }

    pub fn string(string: String) -> Self {
        Self::String(LoxString::string(string))
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Obj::String(s) => write!(f, "\"{}\"", s.string),
            Obj::Class { name: _ } => todo!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoxString {
    pub string: String,
    pub hash: u32,
}

impl LoxString {
    pub fn from_str(s: &str) -> Self {
        Self::string(s.to_owned())
    }

    pub fn string(string: String) -> Self {
        const PRIME: u32 = 16777619;
        let mut hash: u32 = 2166136261;
        for b in string.bytes() {
            hash ^= b as u32;
            hash = hash.wrapping_mul(PRIME);
        }
        Self { string, hash }
    }
}
