use std::ptr;

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

    pub fn write_chunk(&mut self, op_code: OpCode) {
        self.capacity = 8;
        self.code = memory::reallocate(self.code, 0, 8);
        unsafe {
            ptr::write(self.code.add(self.count), op_code);
        }
        self.count += 1;
    }

    pub fn head(&self) -> OpCode {
        unsafe { ptr::read(self.code) }
    }

    pub fn at(&self, position: usize) -> OpCode {
        unsafe { ptr::read(self.code.add(position)) }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

#[cfg(test)]
mod chunk_tests {
    use super::*;

    #[test]
    fn new_works() {
        let chunk = Chunk::new();
        assert_eq!(chunk.capacity(), 0);
        assert_eq!(chunk.count(), 0);
    }

    #[test]
    fn write_chunk() {
        let mut chunk = Chunk::new();
        chunk.write_chunk(OpCode::Return);
        assert_eq!(chunk.capacity(), 8);
        assert_eq!(chunk.count(), 1);
        assert_eq!(chunk.head(), OpCode::Return)
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
        assert_eq!(chunk.count(), 9);
        assert_eq!(chunk.at(8), OpCode::Constant);
        assert_eq!(chunk.capacity(), 16);
    }
}
