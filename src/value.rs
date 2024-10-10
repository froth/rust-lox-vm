use std::fmt::Display;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Value {
    Number(f64),
    Boolean(bool),
    Nil,
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
        }
    }
}
