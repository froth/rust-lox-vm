use std::fmt::Display;

use crate::{
    chunk::Chunk,
    gc::{markable::Markable, Gc},
};

use super::{string::LoxString, upvalue::UpvalueIndex, Hash, Hashable};
#[derive(Debug)]
pub struct Function {
    name: Option<LoxString>,
    arity: u8,
    upvalues: Vec<UpvalueIndex>,
    chunk: Chunk,
}

impl Function {
    pub fn new(
        arity: u8,
        chunk: Chunk,
        name: Option<LoxString>,
        upvalues: Vec<UpvalueIndex>,
    ) -> Self {
        Self {
            arity,
            chunk,
            name,
            upvalues,
        }
    }

    pub fn arity(&self) -> u8 {
        self.arity
    }

    pub fn chunk(&self) -> &Chunk {
        &self.chunk
    }

    pub fn name(&self) -> Option<&LoxString> {
        self.name.as_ref()
    }

    pub fn upvalues(&self) -> &[UpvalueIndex] {
        &self.upvalues
    }
}

impl Markable for Function {
    fn mark(&mut self, gc: &mut Gc) {
        self.chunk.constants.iter_mut().for_each(|c| gc.mark(c));
    }

    fn is_marked(&mut self) -> bool {
        unreachable!()
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
