pub mod alloc;
pub mod markable;
use std::{ops::DerefMut, ptr::NonNull};

use alloc::Alloc;
use markable::Markable;
use tracing::debug;

use crate::{
    datastructures::hash_table::HashTable,
    types::{
        obj::{Obj, ObjStruct},
        obj_ref::ObjRef,
        string::LoxString,
        value::Value,
    },
};

pub struct Gc {
    head: Option<NonNull<Node>>,
    grey: Vec<ObjRef>,
    strings: HashTable,
    bytes_allocated: usize,
    next_gc: usize,
}

pub struct Node {
    next: Option<NonNull<Node>>,
    pub obj_struct: ObjStruct,
}
static GC_GROW_FACTOR: usize = 2;

impl Gc {
    pub fn new() -> Self {
        Self {
            head: None,
            grey: vec![],
            strings: HashTable::new(),
            bytes_allocated: 0,
            next_gc: 1024 * 1024,
        }
    }

    pub fn should_gc(&self) -> bool {
        self.bytes_allocated() > self.next_gc()
    }

    pub fn reset_next_gc(&mut self) {
        self.next_gc = self.bytes_allocated * GC_GROW_FACTOR
    }

    pub fn alloc(&mut self, object: impl Alloc) -> ObjRef {
        object.alloc(self)
    }

    pub fn mark(&mut self, markable: &mut impl Markable) {
        markable.mark(self);
    }

    pub fn trace_references(&mut self) {
        while let Some(mut obj) = self.grey.pop() {
            self.blacken(&mut obj)
        }
    }

    pub fn clean_stringpool(&mut self) {
        self.strings.remove_white();
    }

    pub fn sweep(&mut self) {
        let mut previous: Option<NonNull<Node>> = None;
        let mut object = self.head;
        while let Some(mut o) = object {
            unsafe {
                if o.as_ref().obj_struct.marked {
                    o.as_mut().obj_struct.marked = false;
                    previous = object;
                    object = o.as_ref().next
                } else {
                    let unreached = o;
                    self.free(unreached);
                    object = o.as_ref().next;
                    if let Some(mut prev) = previous {
                        prev.as_mut().next = object;
                    } else {
                        self.head = object;
                    }
                }
            }
        }
    }

    fn grey(&mut self, obj: ObjRef) {
        self.grey.push(obj);
    }

    fn blacken(&mut self, obj: &mut ObjRef) {
        unsafe {
            debug!(
                "{:p} blacken {}",
                obj.0.as_ptr(),
                obj.0.as_ref().obj_struct.obj
            )
        };
        match obj.deref_mut() {
            Obj::Native(_) | Obj::String(_) => (),
            Obj::Function(function) => self.mark(function),
            Obj::Closure { function, upvalues } => {
                self.mark(function);
                upvalues.iter_mut().for_each(|u| self.mark(u));
            }
            Obj::Upvalue {
                location: _,
                next: _,
                closed,
            } => self.mark(closed),
            Obj::Class(_class) => (),
            Obj::Instance { class, fields } => {
                self.mark(class);
                fields.mark(self);
            }
        }
    }

    fn manage_lox_string(&mut self, lox_string: LoxString) -> ObjRef {
        let obj = Obj::String(lox_string);
        let obj_ref = self.add_to_gc(obj);
        // intern the string
        self.strings.insert(Value::Obj(obj_ref), Value::Nil);
        obj_ref
    }

    fn add_to_gc(&mut self, obj: Obj) -> ObjRef {
        self.bytes_allocated += size_of::<Node>();
        unsafe {
            let old_head = self.head.take();
            let new_node = Box::into_raw(Box::new(Node {
                next: old_head,
                obj_struct: ObjStruct::new(obj),
            }));
            let new_node = NonNull::new_unchecked(new_node);
            let obj_ptr: *mut Node = new_node.as_ptr();
            let obj_ref = ObjRef::new(NonNull::new_unchecked(obj_ptr));
            self.head = Some(new_node);
            debug!("{:p} allocate {}", obj_ptr, *obj_ref);
            obj_ref
        }
    }

    fn free(&mut self, ptr: NonNull<Node>) -> Box<Node> {
        self.bytes_allocated -= size_of::<Node>();
        unsafe {
            let node = ptr.as_ptr();
            debug!(
                "{:p} free {}",
                &((*node).obj_struct),
                (*node).obj_struct.obj
            );
            Box::from_raw(node)
        }
    }

    pub fn bytes_allocated(&self) -> usize {
        self.bytes_allocated
    }

    pub fn next_gc(&self) -> usize {
        self.next_gc
    }

    pub fn heapdump(&self) {
        unsafe {
            let mut cur_link = self.head;
            while let Some(boxed_node) = cur_link {
                println!("{:?}", boxed_node.as_ref().obj_struct.obj);
                cur_link = boxed_node.as_ref().next;
            }
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
