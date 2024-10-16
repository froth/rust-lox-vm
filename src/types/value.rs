use std::fmt::Display;

use super::{obj_ref::ObjRef, Hash, Hashable};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Number(f64),
    Boolean(bool),
    Nil,
    Obj(ObjRef),
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        matches!(self, Value::Nil | Value::Boolean(false))
    }
}

impl Hashable for Value {
    fn hash(&self) -> super::Hash {
        match self {
            Value::Number(n) => hash_float(*n),
            Value::Boolean(true) => Hash(3),
            Value::Boolean(false) => Hash(5),
            Value::Nil => Hash(7),
            Value::Obj(obj_ref) => obj_ref.hash(),
        }
    }
}

// taken from
fn hash_float(n: f64) -> Hash {
    #[repr(C)]
    union MyUnion {
        float: f64,
        ints: [u32; 2],
    }
    let union = MyUnion { float: n + 1.0 };
    // SAFETY: for hashing purposes this works fine
    unsafe { Hash(union.ints[0] + union.ints[1]) }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "Nil"),
            Value::Obj(obj) => {
                write!(f, "{}", obj)
            }
        }
    }
}
