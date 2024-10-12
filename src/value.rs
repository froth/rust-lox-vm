use std::{fmt::Display, ops::Deref, ptr::NonNull};

#[derive(Debug, Clone, Copy)]
pub enum Value {
    Number(f64),
    Boolean(bool),
    Nil,
    Obj(NonNull<Obj>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Nil, Self::Nil) => true,
            (Self::Number(l0), Self::Number(r0)) => l0 == r0,
            (Self::Boolean(l0), Self::Boolean(r0)) => l0 == r0,
            (Self::Obj(l0), Self::Obj(r0)) => unsafe { l0.as_ref() == r0.as_ref() },
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Obj {
    String(String),
}

impl Display for Obj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Obj::String(s) => write!(f, "\"{}\"", s),
        }
    }
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

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use crate::value::Value;

    use super::Obj;

    #[test]
    fn string_equal() {
        let mut a = Obj::String(format!("asd{}", "asdasd"));
        let mut b = Obj::String("asdasdasd".to_string());
        let av = Value::Obj(NonNull::new(&mut a).unwrap());
        let bv = Value::Obj(NonNull::new(&mut b).unwrap());
        assert_eq!(av, bv)
    }
}
