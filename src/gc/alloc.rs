use crate::types::{obj::Obj, obj_ref::ObjRef, string::LoxString};

use super::Gc;

pub trait Alloc {
    fn alloc(self, gc: &mut Gc) -> ObjRef;
}

impl Alloc for String {
    fn alloc(self, gc: &mut Gc) -> ObjRef {
        gc.strings
            .find_string(&self)
            .unwrap_or_else(|| gc.manage_lox_string(LoxString::string(self)))
    }
}

impl Alloc for &str {
    fn alloc(self, gc: &mut Gc) -> ObjRef {
        gc.strings
            .find_string(self)
            .unwrap_or_else(|| gc.manage_lox_string(LoxString::from_str(self)))
    }
}

impl Alloc for Obj {
    fn alloc(self, gc: &mut Gc) -> ObjRef {
        gc.add_to_gc(self)
    }
}
