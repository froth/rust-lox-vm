use std::{hint::unreachable_unchecked, ptr::NonNull};

use crate::types::{obj::Obj, obj_ref::ObjRef, string::LoxString};

pub struct Gc {
    head: Option<Box<Node>>,
}

struct Node {
    next: Option<Box<Node>>,
    obj: Obj,
}

impl Gc {
    pub fn new() -> Self {
        Self { head: None }
    }

    pub fn manage(&mut self, obj: Obj) -> ObjRef {
        let old_head = self.head.take();
        let mut new_node = Box::new(Node {
            next: old_head,
            obj,
        });
        let ptr: *mut Obj = &mut new_node.obj;
        self.head = Some(new_node);
        // SAFETY: guaranteed to be not null
        ObjRef::new(unsafe { NonNull::new_unchecked(ptr) })
    }

    pub fn manage_string(&mut self, string: LoxString) -> *const LoxString {
        let old_head = self.head.take();
        let obj = Obj::String(string);
        let mut new_node = Box::new(Node {
            next: old_head,
            obj,
        });
        match &mut new_node.obj {
            Obj::String(s) => {
                // TODO: that is not good
                let ptr: *mut LoxString = s;
                self.head = Some(new_node);
                // SAFETY: guaranteed to be not null
                ptr
            }
            // SAFETY: Obj::String created above
            _ => unsafe { unreachable_unchecked() },
        }
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

    use std::ops::Deref;

    use super::*;

    #[test]
    fn push() {
        let mut gc = Gc::new();
        let one = gc.manage(Obj::from_str("asfsaf"));
        assert_eq!(one.deref(), &Obj::from_str("asfsaf"));
        let two = gc.manage(Obj::from_str("sfdsdfsaf"));
        assert_eq!(two.deref(), &Obj::from_str("sfdsdfsaf"));
        let three = gc.manage(Obj::from_str("sfdsasdasddfsaf"));
        assert_eq!(three.deref(), &Obj::from_str("sfdsasdasddfsaf"));
    }
}
