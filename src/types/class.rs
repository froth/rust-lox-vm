use crate::{
    datastructures::hash_table::HashTable, gc::markable::Markable, types::string::LoxString,
};
use std::fmt::Display;

#[derive(Debug)]
pub struct Class {
    name: LoxString,
    methods: HashTable,
}

impl Class {
    pub fn new(name: LoxString) -> Self {
        Self {
            name,
            methods: HashTable::new(),
        }
    }

    pub fn name(&self) -> &LoxString {
        &self.name
    }
}

impl Markable for Class {
    fn mark(&mut self, gc: &mut crate::gc::Gc) {
        self.methods.mark(gc);
    }

    fn is_marked(&mut self) -> bool {
        unreachable!()
    }
}

impl Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
