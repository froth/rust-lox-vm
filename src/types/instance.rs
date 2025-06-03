use std::fmt::Display;

use crate::{
    datastructures::hash_table::HashTable,
    gc::markable::Markable,
    types::{obj_ref::ObjRef, value::Value, Hashable},
};

#[derive(Debug)]
pub struct Instance {
    class: ObjRef,
    fields: HashTable,
}

impl Instance {
    pub fn new(class: ObjRef) -> Self {
        Self {
            class,
            fields: HashTable::new(),
        }
    }

    pub fn class(&self) -> ObjRef {
        self.class
    }

    pub fn get_field(&self, name: Value) -> Option<Value> {
        self.fields.get(name)
    }

    pub fn set_field(&mut self, name: Value, value: Value) {
        self.fields.insert(name, value);
    }
}

impl Hashable for Instance {
    fn hash(&self) -> crate::types::Hash {
        self.class.hash()
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

impl Display for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} instance", self.class)
    }
}
