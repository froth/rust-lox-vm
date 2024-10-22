use crate::{
    datastructures::hash_table::HashTable,
    types::{obj::Obj, obj_ref::ObjRef, string::LoxString, value::Value},
};

pub struct Gc {
    head: Option<Box<Node>>,
    strings: HashTable,
}

struct Node {
    next: Option<Box<Node>>,
    obj: Obj,
}

impl Gc {
    pub fn new() -> Self {
        Self {
            head: None,
            strings: HashTable::new(),
        }
    }

    pub fn manage_string(&mut self, string: String) -> ObjRef {
        self.strings
            .find_string(&string)
            .unwrap_or_else(|| self.manage_lox_string(LoxString::string(string)))
    }

    pub fn manage_str(&mut self, string: &str) -> ObjRef {
        self.strings
            .find_string(string)
            .unwrap_or_else(|| self.manage_lox_string(LoxString::from_str(string)))
    }

    fn manage_lox_string(&mut self, lox_string: LoxString) -> ObjRef {
        let obj = Obj::String(lox_string);
        let old_head = self.head.take();
        let mut new_node = Box::new(Node {
            next: old_head,
            obj,
        });
        let obj_ref = ObjRef::from_obj(&mut new_node.obj);
        self.head = Some(new_node);
        // intern the string
        self.strings.insert(Value::Obj(obj_ref), Value::Nil);
        obj_ref
    }
}

impl Drop for Gc {
    fn drop(&mut self) {
        let mut cur_link = self.head.take();
        while let Some(mut boxed_node) = cur_link {
            cur_link = boxed_node.next.take();
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn push() {
        let mut gc = Gc::new();
        let one = gc.manage_str("asfsaf");
        assert_eq!(*one, Obj::String(LoxString::from_str("asfsaf")));
        let two = gc.manage_str("sfdsdfsaf");
        assert_eq!(*two, Obj::String(LoxString::from_str("sfdsdfsaf")));
        let three = gc.manage_str("sfdsasdasddfsaf");
        assert_eq!(*three, Obj::String(LoxString::from_str("sfdsasdasddfsaf")));
    }

    #[test]
    fn string_interning() {
        let mut gc = Gc::new();
        let one = gc.manage_str("asfsaf");
        let two = gc.manage_str("asfsaf");
        assert_eq!(one, two);
    }
}
