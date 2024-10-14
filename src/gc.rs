use std::ptr::NonNull;

use crate::value::Obj;

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

    pub fn manage(&mut self, obj: Obj) -> NonNull<Obj> {
        let old_head = self.head.take();
        let mut new_node = Box::new(Node {
            next: old_head,
            obj,
        });
        let ptr: *mut Obj = &mut new_node.obj;
        self.head = Some(new_node);
        // SAFETY: guaranteed to be not null
        unsafe { NonNull::new_unchecked(ptr) }
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
        let one = gc.manage(Obj::from_str("asfsaf"));
        let one = unsafe { one.as_ref() };
        assert_eq!(one, &Obj::from_str("asfsaf"));
        let two = gc.manage(Obj::from_str("sfdsdfsaf"));
        let two = unsafe { two.as_ref() };
        assert_eq!(two, &Obj::from_str("sfdsdfsaf"));
        let three = gc.manage(Obj::from_str("sfdsasdasddfsaf"));
        let three = unsafe { three.as_ref() };
        assert_eq!(three, &Obj::from_str("sfdsasdasddfsaf"));
    }
}
