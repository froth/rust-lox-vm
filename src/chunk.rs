use crate::lox_vector::LoxVector;

#[derive(Debug, PartialEq)]
pub enum OpCode {
    Return,
    Constant,
}

pub struct Chunk {
    code: LoxVector<OpCode>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: LoxVector::new(),
        }
    }

    pub fn write_chunk(&mut self, op_code: OpCode) {
        self.code.push(op_code)
    }

    pub fn disassemble(&self, name: &str) {
        eprintln!("== {} ==", name);
        let mut iter = self.code.iter().enumerate();
        while let Some((offset, instruction)) = iter.next() {
            eprint!("{:0>4} ", offset);
            match instruction {
                OpCode::Return => eprintln!("RETURN"),
                OpCode::Constant => eprintln!("CONSTANT"),
            }
        }
    }
}
