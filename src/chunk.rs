use miette::{NamedSource, SourceCode, SourceSpan};

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
    spans: LoxVector<SourceSpan>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: LoxVector::new(),
            constants: LoxVector::new(),
            spans: LoxVector::new(),
        }
    }

    pub fn write(&mut self, byte: u8, span: SourceSpan) {
        self.code.push(byte);
        self.spans.push(span);
    }

    pub fn write_op_code(&mut self, op_code: OpCode, span: SourceSpan) {
        self.write(op_code as u8, span);
    }

    pub fn add_constant(&mut self, value: Value) -> u8 {
        self.constants.push(value);
        (self.constants.len() - 1)
            .try_into()
            .expect("constant count overflows u8, not supported")
    }

    pub fn disassemble<T: miette::SourceCode>(&self, source: NamedSource<T>) {
        eprintln!("== {} ==", source.name());
        let mut iter = self.code.iter().zip(self.spans.iter()).enumerate();
        let mut last_line_number = None;
        while let Some((offset, (byte_code, span))) = iter.next() {
            eprint!("{:0>4} ", offset);
            let line_number = source.read_span(span, 0, 0).unwrap().line();
            if last_line_number.is_some_and(|l| l == line_number) {
                eprint!("   | ");
            } else {
                eprint!("{:>4} ", line_number + 1);
                last_line_number = Some(line_number);
            }
            let instruction = OpCode::try_from(*byte_code).unwrap();
            match instruction {
                OpCode::Return => eprintln!("RETURN"),
                OpCode::Constant => {
                    let (_, (const_index, _)) = iter.next().expect("CONSTANT without const index");
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
