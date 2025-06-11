use std::ops::Deref;

use miette::SourceSpan;

use crate::{
    chunk::Chunk,
    op::Op,
    types::{function::Function, obj::Obj, obj_ref::ObjRef, value::Value},
};

pub(super) struct CallFrame {
    pub(super) closure: ObjRef,
    pub(super) ip: *const Op,
    pub(super) slots: *mut Value,
}

impl CallFrame {
    pub(super) fn function(&self) -> &Function {
        if let Obj::Closure(closure) = self.closure.deref() {
            if let Obj::Function(function) = closure.function.deref() {
                return function;
            }
        }
        unreachable!("callframe stored non-closure")
    }

    pub(super) fn upvalues(&self) -> &[ObjRef] {
        if let Obj::Closure(closure) = self.closure.deref() {
            &closure.upvalues
        } else {
            unreachable!("callframe stored non-closure")
        }
    }

    pub(super) fn chunk(&self) -> &Chunk {
        self.function().chunk()
    }
    fn current_index(&self) -> usize {
        unsafe { self.ip.offset_from(&(*self.function()).chunk().code[0]) as usize }
    }

    pub(super) fn current_location(&self) -> SourceSpan {
        self.chunk().locations[self.current_index()]
    }

    pub(super) fn disassemble_at_current_index(&mut self) -> String {
        let current_index = self.current_index();
        self.chunk().disassemble_at(current_index)
    }
}
