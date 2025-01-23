use std::fmt::Display;

use crate::chunk::Chunk;

use super::{string::LoxString, Hash, Hashable};

pub struct Function {
    arity: usize,
    chunk: Chunk,
    name: Option<LoxString>,
}

impl Function {
    pub fn new(arity: usize, chunk: Chunk, name: Option<LoxString>) -> Self {
        Self { arity, chunk, name }
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn chunk(&self) -> &Chunk {
        &self.chunk
    }

    pub fn name(&self) -> Option<&LoxString> {
        self.name.as_ref()
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
