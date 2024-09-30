use miette::{NamedSource, SourceCode, SourceSpan};
use std::fmt::Write as _;

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

    pub fn disassemble<T: SourceCode>(&self, source: &NamedSource<T>) {
        eprintln!("== {} ==", source.name());
        let mut iter = self.code.iter().zip(self.spans.iter()).enumerate();
        let mut last_line_number = None;
        while let Some((disassembled, line_number)) =
            self.disassemble_next(&mut iter, source, last_line_number)
        {
            eprintln!("{disassembled}");
            last_line_number = Some(line_number);
        }
    }

    pub fn disassemble_at<T: SourceCode>(&self, source: &NamedSource<T>, at: usize) -> String {
        let mut iter = self.code.iter().zip(self.spans.iter()).enumerate();
        if at == 0 {
            let (result, _) = self
                .disassemble_next(&mut iter, source, None)
                .expect("disassambling unknown index");
            result
        } else {
            let mut skiped_iter = iter.skip(at - 1);
            let (_, (_, span)) = skiped_iter.next().expect("disassembling unknown index");
            let last_line_number = source.read_span(span, 0, 0).unwrap().line();
            let (result, _) = self
                .disassemble_next(&mut skiped_iter, source, Some(last_line_number))
                .expect("disassambling unknown index");
            result
        }
    }

    fn disassemble_next<'a, T: SourceCode>(
        &self,
        iter: &mut impl Iterator<Item = (usize, (&'a u8, &'a SourceSpan))>,
        source: &NamedSource<T>,
        last_line_number: Option<usize>,
    ) -> Option<(String, usize)> {
        let mut result = String::new();
        let (offset, (byte_code, span)) = iter.next()?;
        let line_number = source.read_span(span, 0, 0).unwrap().line();

        let _ = write!(&mut result, "{offset:0>4} ");

        if last_line_number.is_some_and(|l| l == line_number) {
            let _ = write!(&mut result, "   | ");
        } else {
            let _ = write!(&mut result, "{:>4} ", line_number + 1);
        }

        let instruction =
            OpCode::try_from(*byte_code).unwrap_or_else(|_| panic!("Unknown OpCode {}", byte_code));
        match instruction {
            OpCode::Return => {
                let _ = write!(&mut result, "RETURN");
            }
            OpCode::Constant => {
                let (_, (const_index, _)) = iter.next().expect("CONSTANT without const index");
                let const_index: usize = (*const_index).into();
                let constant = self.constants[const_index];
                let _ = write!(
                    &mut result,
                    "{:<16} {:<4} '{}'",
                    "CONSTANT", const_index, constant
                );
            }
        }
        Some((result, line_number))
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
