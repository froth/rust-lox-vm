use std::fmt::Display;

use super::{string::LoxString, Hashable};

#[derive(Debug, Clone, PartialEq)]
pub enum Obj {
    String(LoxString),
    Class { name: LoxString },
}

impl Hashable for Obj {
    fn hash(&self) -> super::Hash {
        match self {
            Obj::String(lox_string) => lox_string.hash(),
            Obj::Class { name } => name.hash(),
        }
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Obj::String(s) => write!(f, "{}", s.string),
            Obj::Class { name: _ } => todo!(),
        }
    }
}
