use tracing::debug;

use crate::{gc::GcAlloc, types::obj_ref::ObjRef};

use super::VM;

impl VM {
    pub fn collect_garbage(&mut self) {
        debug!("gc begin");
        debug!("gc end");
    }

    pub(super) fn alloc(&mut self, object: impl GcAlloc) -> ObjRef {
        #[cfg(feature = "stress_gc")]
        self.collect_garbage();
        self.gc.alloc(object)
    }
}
