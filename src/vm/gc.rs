use tracing::debug;

use crate::{gc::alloc::Alloc, types::obj_ref::ObjRef};

use super::{UpvalueLocation, VM};
// Mostly separated to get better scoping for tracing targets
impl VM {
    pub fn collect_garbage(&mut self) {
        debug!("gc begin");
        let before = self.gc.bytes_allocated();

        self.mark_roots();
        self.gc.trace_references();
        self.gc.clean_stringpool();
        self.gc.sweep();
        self.gc.reset_next_gc();

        debug!("gc end");
        debug!(
            "collected {} bytes (from {} to {}) next at {}",
            before - self.gc.bytes_allocated(),
            before,
            self.gc.bytes_allocated(),
            self.gc.next_gc()
        );
    }

    pub(super) fn alloc(&mut self, object: impl Alloc) -> ObjRef {
        #[cfg(feature = "stress_gc")]
        self.collect_garbage();

        if self.gc.should_gc() {
            self.collect_garbage();
        }
        self.gc.alloc(object)
    }

    fn mark_roots(&mut self) {
        let mut stack_ptr = self.stack;
        while stack_ptr < self.stack_top {
            (unsafe {
                self.gc.mark(&mut *stack_ptr);
            });
            stack_ptr = unsafe { stack_ptr.add(1) };
        }

        for i in 0..self.frame_count {
            unsafe { self.gc.mark(&mut (*self.frames.add(i)).closure) };
        }

        let mut upvalue = self.open_upvalues;
        while let Some(UpvalueLocation {
            location: _,
            mut current,
            next,
        }) = Self::upvalue_location(upvalue)
        {
            self.gc.mark(&mut current);
            upvalue = next;
        }

        self.globals.mark(&mut self.gc);
        self.gc.mark(&mut self.init_string);
    }
}
