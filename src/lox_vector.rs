use std::{
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
};

use crate::memory;

/// Custom Vector type.
/// In clox several special purpose dynamic arrays exist, I made a generic type as it is easy enough in Rust.
/// I did not use `std::vec` because implementing the datastructures are part of the book
/// and because the GC uses `crate::memory` to get information about memory consumption
pub struct LoxVector<T> {
    count: usize,
    capacity: usize,
    ptr: NonNull<T>,
}

impl<T> LoxVector<T> {
    pub fn new() -> Self {
        assert!(
            std::mem::size_of::<T>() != 0,
            "We're not ready to handle ZSTs"
        );
        Self {
            ptr: NonNull::dangling(),
            count: 0,
            capacity: 0,
        }
    }

    fn grow(&mut self) {
        let new_capacity = memory::grow_capacity(self.capacity);
        self.ptr = memory::reallocate(self.ptr, self.capacity, new_capacity);
        self.capacity = new_capacity;
    }

    pub fn push(&mut self, value: T) {
        if self.count == self.capacity {
            self.grow()
        }
        unsafe {
            ptr::write(self.ptr.as_ptr().add(self.count), value);
        }
        self.count += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.count == 0 {
            None
        } else {
            self.count -= 1;
            unsafe { Some(ptr::read(self.ptr.as_ptr().add(self.count))) }
        }
    }
}

impl<T> Drop for LoxVector<T> {
    fn drop(&mut self) {
        if self.capacity != 0 {
            while self.pop().is_some() {}
            memory::reallocate(self.ptr, self.capacity, 0);
        }
    }
}

impl<T> Deref for LoxVector<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        // handle null pointer
        if self.count == 0 {
            return &[];
        }
        // SAFETY:
        // properly aligned slice of memory is guaranteed by grow
        // count guarantees that there are valid OpCodes in all this memory
        // mutation can only take place with properly annotated &mut
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.count) }
    }
}

impl<T> DerefMut for LoxVector<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        // handle null pointer
        if self.count == 0 {
            return &mut [];
        }
        // SAFETY:
        // properly aligned slice of memory is guaranteed by grow
        // count guarantees that there are valid OpCodes in all this memory
        // mutation can only take place with properly annotated &mut
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.count) }
    }
}

#[cfg(test)]
mod tests {
    use crate::chunk::OpCode;

    use super::*;

    #[test]
    fn new_works() {
        let vec: LoxVector<usize> = LoxVector::new();
        assert_eq!(vec.capacity, 0);
        assert_eq!(vec.len(), 0);
    }

    #[test]
    fn write_chunk() {
        let mut vec = LoxVector::new();
        vec.push(OpCode::Return);
        assert_eq!(vec.capacity, 8);
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0], OpCode::Return)
    }

    #[test]
    fn grow() {
        let mut vec = LoxVector::new();
        vec.push(OpCode::Return);
        vec.push(OpCode::Return);
        vec.push(OpCode::Return);
        vec.push(OpCode::Return);
        vec.push(OpCode::Return);
        vec.push(OpCode::Return);
        vec.push(OpCode::Return);
        vec.push(OpCode::Return);
        vec.push(OpCode::Constant);
        assert_eq!(vec.len(), 9);
        assert_eq!(vec[8], OpCode::Constant);
        assert_eq!(vec.capacity, 16);
    }

    #[test]
    fn slices_work() {
        let mut vec = LoxVector::new();
        vec.push(OpCode::Return);
        vec[0] = OpCode::Constant;
        assert_eq!(vec[0], OpCode::Constant);
    }
}
