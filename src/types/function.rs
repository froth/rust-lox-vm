use std::fmt::Display;

use crate::chunk::Chunk;

use super::{string::LoxString, Hash, Hashable};

pub struct Function {
    arity: u8,
    chunk: Chunk,
    name: Option<LoxString>,
}

impl Function {
    pub fn new(arity: u8, chunk: Chunk, name: Option<LoxString>) -> Self {
        Self { arity, chunk, name }
    }

    pub fn arity(&self) -> u8 {
        self.arity
    }

    pub fn chunk(&self) -> &Chunk {
        &self.chunk
    }
}

impl Hashable for Function {
    fn hash(&self) -> super::Hash {
        match self {
            Function {
                name: Some(name), ..
            } => name.hash(),
            Function { name: None, .. } => Hash(11),
        }
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Function {
                name: Some(name), ..
            } => write!(f, "<fn {}>", name.string),
            Function { name: None, .. } => write!(f, "<script>"),
        }
    }
}
