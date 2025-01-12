use miette::{NamedSource, SourceCode, SourceSpan};
use std::fmt::{Error, Write as _};

use crate::{datastructures::vector::LoxVector, op::Op, types::value::Value};

pub struct Chunk {
    // in original clox this is Vector<u8> this is more wasteful but way easier. Maybe benchmark in the future?
    pub code: LoxVector<Op>,
    pub constants: LoxVector<Value>,
    pub locations: LoxVector<SourceSpan>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: LoxVector::new(),
            constants: LoxVector::new(),
            locations: LoxVector::new(),
        }
    }

    pub fn write(&mut self, op: Op, location: SourceSpan) {
        self.code.push(op);
        self.locations.push(location);
    }

    pub fn add_constant(&mut self, value: Value) -> u8 {
        self.constants.push(value);
        (self.constants.len() - 1)
            .try_into()
            .expect("constant count overflows u8, not supported")
    }

    pub fn disassemble<T: SourceCode>(&self, source: &NamedSource<T>) -> String {
        let mut result = String::new();
        let _ = writeln!(&mut result, "== {} ==", source.name());
        let iter = self.code.iter().zip(self.locations.iter()).enumerate();
        let mut last_line_number = None;

        for (offset, (op, span)) in iter {
            let (disassembled, line_number) = self
                .to_disassembled(offset, op, span, source, last_line_number)
                .expect("writing to String can't fail");
            let _ = writeln!(&mut result, "{disassembled}");
            last_line_number = Some(line_number);
        }
        result
    }

    pub fn disassemble_at<T: SourceCode>(&self, source: &NamedSource<T>, at: usize) -> String {
        let mut iter = self.code.iter().zip(self.locations.iter()).enumerate();
        if at == 0 {
            let (offset, (op, span)) = iter.next().expect("trying to disassemble empty chunk");
            let (disassembled, _) = self
                .to_disassembled(offset, op, span, source, None)
                .expect("writing to string can't fail");
            disassembled
        } else {
            let mut skiped_iter = iter.skip(at - 1);
            let (_, (_, span)) = skiped_iter.next().expect("disassembling unknown index");
            let last_line_number = source.read_span(span, 0, 0).unwrap().line();
            let (offset, (op, span)) = skiped_iter
                .next()
                .expect("trying to disassemble empty chunk");
            let (disassembled, _) = self
                .to_disassembled(offset, op, span, source, Some(last_line_number))
                .expect("writing to string can't fail");
            disassembled
        }
    }

    fn to_disassembled<T>(
        &self,
        offset: usize,
        op: &Op,
        span: &SourceSpan,
        source: &NamedSource<T>,
        last_line_number: Option<usize>,
    ) -> Result<(String, usize), Error>
    where
        T: SourceCode,
    {
        let mut result = String::new();
        let line_number = source.read_span(span, 0, 0).unwrap().line();

        write!(&mut result, "{offset:0>4} ")?;

        if last_line_number.is_some_and(|l| l == line_number) {
            write!(&mut result, "   | ")?;
        } else {
            write!(&mut result, "{:>4} ", line_number + 1)?;
        }

        match op {
            Op::Constant(idx) | Op::DefineGlobal(idx) | Op::GetGlobal(idx) | Op::SetGlobal(idx) => {
                let const_index: usize = (*idx).into();
                let constant = self.constants[const_index];
                write!(&mut result, "{:<16} {:<4} '{}'", op, const_index, constant)?;
            }
            Op::GetLocal(slot) | Op::SetLocal(slot) => {
                write!(&mut result, "{:<16} {:<4}", op, slot)?
            }
            Op::JumpIfFalse(jump) | Op::Jump(jump) => write!(
                &mut result,
                "{:<16} {:0>4} -> {:0>4}",
                op,
                offset,
                offset + (*jump as usize)
            )?,
            op => write!(&mut result, "{op}")?,
        }
        Ok((result, line_number))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn constant_index() {
        let mut chunk: Chunk = Chunk::new();
        let index = chunk.add_constant(Value::Number(12.1));
        assert_eq!(index, 0);
        let index = chunk.add_constant(Value::Number(12.1));
        assert_eq!(index, 1)
    }

    #[test]
    fn disassemble_constant() {
        let mut chunk = Chunk::new();
        let constant = chunk.add_constant(Value::Number(1.1));
        let src = "1.1";
        let src = NamedSource::new("src", src);
        chunk.write(Op::Constant(constant), SourceSpan::from((0, 3)));
        let res = chunk.disassemble_at(&src, 0);
        assert_eq!(res, "0000    1 CONSTANT         0    '1.1'");
    }
}
