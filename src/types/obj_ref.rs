use std::{
    fmt::{Debug, Display},
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::gc::Node;

use super::obj::Obj;

#[derive(Clone, Copy, PartialEq)]
pub struct ObjRef(pub NonNull<Node>);

impl ObjRef {
    pub fn new(ptr: NonNull<Node>) -> Self {
        ObjRef(ptr)
    }
}

impl Deref for ObjRef {
    type Target = Obj;

    fn deref(&self) -> &Self::Target {
        // SAFETY ptr is guaranteed to be managed by GC and has proper alignment and type
        unsafe { &self.0.as_ref().obj_struct.obj }
    }
}

impl DerefMut for ObjRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY ptr is guaranteed to be managed by GC and has proper alignment and type
        unsafe { &mut self.0.as_mut().obj_struct.obj }
    }
}

impl Display for ObjRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.deref())
    }
}

impl Debug for ObjRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.deref())
    }
}
