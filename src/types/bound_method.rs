use std::fmt::{Debug, Display};

use crate::{
    gc::{markable::Markable, Gc},
    types::{closure::Closure, obj_ref::ObjRef, string::LoxString, value::Value, Hash, Hashable},
};

pub struct BoundMethod {
    receiver: Value,
    method: ObjRef,
}

impl BoundMethod {
    pub fn new(receiver: Value, method: ObjRef) -> Self {
        Self { receiver, method }
    }

    pub fn method(&self) -> &Closure {
        self.method.as_closure()
    }
}

impl Markable for BoundMethod {
    fn mark(&mut self, gc: &mut Gc) {
        self.receiver.mark(gc);
        self.method.mark(gc);
    }

    fn is_marked(&mut self) -> bool {
        unreachable!()
    }
}

impl Hashable for BoundMethod {
    fn hash(&self) -> Hash {
        // Combine hashes of receiver and method
        let receiver_hash = self.receiver.hash();
        let method_hash = self.method.hash();
        Hash((receiver_hash.0.wrapping_mul(31)).wrapping_add(method_hash.0))
    }
}

impl Debug for BoundMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundMethod")
            .field("receiver", &self.receiver)
            .field("method", &self.method)
            .finish()
    }
}

impl Display for BoundMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.method()
                .function
                .as_function()
                .name()
                .expect("can only be named functions")
        )
    }
}
