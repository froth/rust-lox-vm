use std::fmt::{Debug, Display};

use crate::{
    gc::{markable::Markable, Gc},
    types::{obj_ref::ObjRef, Hash, Hashable},
};

pub struct Closure {
    pub function: ObjRef,
    pub upvalues: Vec<ObjRef>,
}

impl Closure {
    pub fn new(function: ObjRef, upvalues: Vec<ObjRef>) -> Self {
        Self { function, upvalues }
    }
}

impl Display for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "closure over {}", self.function)
    }
}

impl Hashable for Closure {
    fn hash(&self) -> Hash {
        self.function.hash()
    }
}

impl Markable for Closure {
    fn mark(&mut self, gc: &mut Gc) {
        self.function.mark(gc);
        self.upvalues.iter_mut().for_each(|u| u.mark(gc));
    }

    fn is_marked(&mut self) -> bool {
        unreachable!()
    }
}

impl Debug for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let function_name = self.function.as_function().name();
        f.debug_struct("Closure")
            .field("function", &function_name)
            .field("upvalues", &self.upvalues)
            .finish()
    }
}
