use crate::{lox_vector::LoxVector, value::Value};

#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum OpCode {
    Return,
    Constant,
}

#[derive(Debug)]
pub struct BadOpCode;

impl TryFrom<u8> for OpCode {
    type Error = BadOpCode;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        const RETURN: u8 = OpCode::Return as u8;
        const CONSTANT: u8 = OpCode::Constant as u8;
        match value {
            RETURN => Ok(OpCode::Return),
            CONSTANT => Ok(OpCode::Constant),
            _ => Err(BadOpCode),
        }
    }
}

pub struct Chunk {
    code: LoxVector<u8>,
    constants: LoxVector<Value>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: LoxVector::new(),
            constants: LoxVector::new(),
        }
    }

    pub fn write(&mut self, byte: u8) {
        self.code.push(byte)
    }

    pub fn write_op_code(&mut self, op_code: OpCode) {
        self.code.push(op_code as u8)
    }

    pub fn add_constant(&mut self, value: Value) -> u8 {
        self.constants.push(value);
        (self.constants.len() - 1)
            .try_into()
            .expect("constant count overflows u8, not supported")
    }

    pub fn disassemble(&self, name: &str) {
        eprintln!("== {} ==", name);
        let mut iter = self.code.iter().enumerate();
        while let Some((offset, byte_code)) = iter.next() {
            eprint!("{:0>4} ", offset);
            let instruction = OpCode::try_from(*byte_code).unwrap();
            match instruction {
                OpCode::Return => eprintln!("RETURN"),
                OpCode::Constant => {
                    let (_, const_index) =
                        iter.next().expect("CONSTANT without following const index");
                    let const_index: usize = (*const_index).into();
                    let constant = self.constants[const_index];
                    eprintln!("{:<16} {:<4} '{}'", "CONSTANT", const_index, constant)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn constant_index() {
        let mut chunk: Chunk = Chunk::new();
        let index = chunk.add_constant(12.1);
        assert_eq!(index, 0);
        let index = chunk.add_constant(12.1);
        assert_eq!(index, 1)
    }
}
