use std::{
    ops::{Deref, DerefMut},
    ptr,
};

use crate::memory;

#[derive(Debug, PartialEq)]
pub enum OpCode {
    Return,
    Constant,
}

pub struct Chunk {
    count: usize,
    capacity: usize,
    code: *mut OpCode,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: ptr::null_mut(),
            count: 0,
            capacity: 0,
        }
    }

    fn grow(&mut self) {
        let new_capacity = memory::grow_capacity(self.capacity);
        self.code = memory::reallocate(self.code, self.capacity, new_capacity);
        self.capacity = new_capacity;
    }

    pub fn write_chunk(&mut self, op_code: OpCode) {
        if self.count == self.capacity {
            self.grow()
        }
        unsafe {
            ptr::write(self.code.add(self.count), op_code);
        }
        self.count += 1;
    }

    pub fn clear(&mut self) {
        self.code = memory::reallocate(self.code, self.capacity, 0);
        self.capacity = 0;
        self.count = 0;
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        self.code = memory::reallocate(self.code, self.capacity, 0);
    }
}

impl Deref for Chunk {
    type Target = [OpCode];
    fn deref(&self) -> &[OpCode] {
        // SAFETY:
        // properly aligned slice of memory is guaranteed by grow
        // count guarantees that there are valid OpCodes in all this memory
        // mutation can only take place with properly annotated &mut
        unsafe { std::slice::from_raw_parts(self.code, self.count) }
    }
}

impl DerefMut for Chunk {
    fn deref_mut(&mut self) -> &mut [OpCode] {
        // SAFETY:
        // properly aligned slice of memory is guaranteed by grow
        // count guarantees that there are valid OpCodes in all this memory
        // mutation can only take place with properly annotated &mut
        unsafe { std::slice::from_raw_parts_mut(self.code, self.count) }
    }
}

#[cfg(test)]
mod chunk_tests {
    use std::mem;

    use super::*;

    #[test]
    fn opcode_does_not_need_drop() {
        assert!(!mem::needs_drop::<OpCode>());
    }

    #[test]
    fn new_works() {
        let chunk = Chunk::new();
        assert_eq!(chunk.capacity(), 0);
        assert_eq!(chunk.len(), 0);
    }

    #[test]
    fn write_chunk() {
        let mut chunk = Chunk::new();
        chunk.write_chunk(OpCode::Return);
        assert_eq!(chunk.capacity(), 8);
        assert_eq!(chunk.len(), 1);
        assert_eq!(chunk[0], OpCode::Return)
    }

    #[test]
    fn grow() {
        let mut chunk = Chunk::new();
        chunk.write_chunk(OpCode::Return);
        chunk.write_chunk(OpCode::Return);
        chunk.write_chunk(OpCode::Return);
        chunk.write_chunk(OpCode::Return);
        chunk.write_chunk(OpCode::Return);
        chunk.write_chunk(OpCode::Return);
        chunk.write_chunk(OpCode::Return);
        chunk.write_chunk(OpCode::Return);
        chunk.write_chunk(OpCode::Constant);
        assert_eq!(chunk.len(), 9);
        assert_eq!(chunk[8], OpCode::Constant);
        assert_eq!(chunk.capacity(), 16);
    }

    #[test]
    fn clear() {
        let mut chunk = Chunk::new();
        chunk.write_chunk(OpCode::Return);
        chunk.clear();
        assert_eq!(chunk.len(), 0);
        assert_eq!(chunk.capacity(), 0);
    }

    #[test]
    fn slices_work() {
        let mut chunk = Chunk::new();
        chunk.write_chunk(OpCode::Return);
        chunk[0] = OpCode::Constant;
        assert_eq!(chunk[0], OpCode::Constant);
    }
}
