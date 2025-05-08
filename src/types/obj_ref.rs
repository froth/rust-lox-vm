use std::{
    fmt::Display,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use super::obj::Obj;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjRef(NonNull<Obj>);

impl ObjRef {
    pub fn new(ptr: NonNull<Obj>) -> Self {
        ObjRef(ptr)
    }
}

impl Deref for ObjRef {
    type Target = Obj;

    fn deref(&self) -> &Self::Target {
        // SAFETY ptr is guaranteed to be managed by GC and has proper alignment and type
        unsafe { self.0.as_ref() }
    }
}

impl DerefMut for ObjRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY ptr is guaranteed to be managed by GC and has proper alignment and type
        unsafe { self.0.as_mut() }
    }
}

impl Display for ObjRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.deref())
    }
}
