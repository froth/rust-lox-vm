use std::ptr::NonNull;

use tracing::debug;

use crate::{
    datastructures::hash_table::HashTable,
    types::{obj::Obj, obj_ref::ObjRef, string::LoxString, value::Value},
};

pub struct Gc {
    head: Option<NonNull<Node>>,
    strings: HashTable,
}

struct Node {
    next: Option<NonNull<Node>>,
    obj: Obj,
}

impl Gc {
    pub fn new() -> Self {
        Self {
            head: None,
            strings: HashTable::new(),
        }
    }

    pub fn alloc(&mut self, object: impl GcAlloc) -> ObjRef {
        object.alloc(self)
    }

    fn manage_lox_string(&mut self, lox_string: LoxString) -> ObjRef {
        let obj = Obj::String(lox_string);
        let obj_ref = self.add_to_gc(obj);
        // intern the string
        self.strings.insert(Value::Obj(obj_ref), Value::Nil);
        obj_ref
    }

    fn add_to_gc(&mut self, obj: Obj) -> ObjRef {
        unsafe {
            let old_head = self.head.take();
            let new_node = Box::into_raw(Box::new(Node {
                next: old_head,
                obj,
            }));
            let new_node = NonNull::new_unchecked(new_node);
            let obj_ptr: *mut Obj = &mut (*new_node.as_ptr()).obj;
            let obj_ref = ObjRef::new(NonNull::new_unchecked(obj_ptr));
            self.head = Some(new_node);
            debug!("{:p} allocate {}", obj_ptr, *obj_ref);
            obj_ref
        }
    }

    fn free(&mut self, ptr: NonNull<Node>) -> Box<Node> {
        unsafe {
            let node = ptr.as_ptr();
            debug!("{:p} free {}", &((*node).obj), (*node).obj);
            Box::from_raw(node)
        }
    }
}

impl Drop for Gc {
    fn drop(&mut self) {
        let mut cur_link = self.head.take();
        while let Some(boxed_node) = cur_link {
            cur_link = self.free(boxed_node).next.take();
        }
    }
}

pub trait GcAlloc {
    fn alloc(self, gc: &mut Gc) -> ObjRef;
}

impl GcAlloc for String {
    fn alloc(self, gc: &mut Gc) -> ObjRef {
        gc.strings
            .find_string(&self)
            .unwrap_or_else(|| gc.manage_lox_string(LoxString::string(self)))
    }
}

impl GcAlloc for &str {
    fn alloc(self, gc: &mut Gc) -> ObjRef {
        gc.strings
            .find_string(self)
            .unwrap_or_else(|| gc.manage_lox_string(LoxString::from_str(self)))
    }
}

impl GcAlloc for Obj {
    fn alloc(self, gc: &mut Gc) -> ObjRef {
        gc.add_to_gc(self)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn push() {
        let mut gc = Gc::new();
        let one = gc.alloc("asfsaf");
        assert_eq!(one.to_string(), "asfsaf");
        let two = gc.alloc("sfdsdfsaf");
        assert_eq!(two.to_string(), "sfdsdfsaf");
        let three = gc.alloc("sfdsasdasddfsaf");
        assert_eq!(three.to_string(), "sfdsasdasddfsaf");
    }

    #[test]
    fn string_interning() {
        let mut gc = Gc::new();
        let one = gc.alloc("asfsaf");
        let two = gc.alloc("asfsaf");
        assert_eq!(one, two);
    }
}
