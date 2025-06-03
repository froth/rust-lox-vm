use std::fmt::Display;

use crate::{
    datastructures::hash_table::HashTable,
    gc::markable::Markable,
    types::{obj_ref::ObjRef, Hashable},
};

#[derive(Debug)]
pub struct Instance {
    pub class: ObjRef,
    pub fields: HashTable,
}

impl Instance {
    pub fn new(class: ObjRef) -> Self {
        Self {
            class,
            fields: HashTable::new(),
        }
    }
}

impl Markable for Instance {
    fn mark(&mut self, gc: &mut crate::gc::Gc) {
        self.class.mark(gc);
        self.fields.mark(gc);
    }

    fn is_marked(&mut self) -> bool {
        unreachable!()
    }
}

impl Hashable for Instance {
    fn hash(&self) -> crate::types::Hash {
        self.class.hash()
    }
}

impl Display for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} instance", self.class)
    }
}
