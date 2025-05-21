use tracing::debug;

use crate::types::{obj_ref::ObjRef, value::Value};

use super::Gc;

pub trait Markable {
    fn mark(&mut self, gc: &mut Gc);
    fn is_marked(&mut self) -> bool;
}

impl Markable for ObjRef {
    fn mark(&mut self, gc: &mut Gc) {
        unsafe {
            debug!("{:p} mark {}", self.0.as_ptr(), self.0.as_ref().obj);
            if self.0.as_ref().marked {
                return;
            }
            self.0.as_mut().marked = true;
            gc.grey(*self);
        }
    }

    fn is_marked(&mut self) -> bool {
        unsafe { self.0.as_ref().marked }
    }
}
impl Markable for Value {
    fn mark(&mut self, gc: &mut Gc) {
        if let Value::Obj(obj) = self {
            obj.mark(gc)
        }
    }

    fn is_marked(&mut self) -> bool {
        if let Value::Obj(obj) = self {
            unsafe { obj.0.as_ref().marked }
        } else {
            true
        }
    }
}
