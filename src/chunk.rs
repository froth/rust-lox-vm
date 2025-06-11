use miette::{NamedSource, SourceCode, SourceSpan};
use std::{
    fmt::{Debug, Error, Write as _},
    ops::Deref,
    sync::Arc,
};

use crate::{
    datastructures::vector::LoxVector,
    op::Op,
    types::{obj::Obj, value::Value},
};

pub struct Chunk {
    // in original clox this is Vector<u8> this is more wasteful but way easier. Maybe benchmark in the future?
    pub code: LoxVector<Op>,
    pub constants: LoxVector<Value>,
    pub locations: LoxVector<SourceSpan>,
    pub source: Arc<NamedSource<String>>,
}

impl Chunk {
    pub fn new(source: Arc<NamedSource<String>>) -> Self {
        Self {
            code: LoxVector::new(),
            constants: LoxVector::new(),
            locations: LoxVector::new(),
            source,
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

    pub fn disassemble(&self) -> String {
        let mut result = String::new();
        let _ = writeln!(&mut result, "== {} ==", self.source.name());
        let iter = self.code.iter().zip(self.locations.iter()).enumerate();
        let mut last_line_number = None;

        for (offset, (op, span)) in iter {
            let (disassembled, line_number) = self
                .to_disassembled(offset, op, span, last_line_number)
                .expect("writing to String can't fail");
            let _ = writeln!(&mut result, "{disassembled}");
            last_line_number = Some(line_number);
        }
        result
    }

    pub fn disassemble_at(&self, at: usize) -> String {
        let mut iter = self.code.iter().zip(self.locations.iter()).enumerate();
        if at == 0 {
            let (offset, (op, span)) = iter.next().expect("trying to disassemble empty chunk");
            let (disassembled, _) = self
                .to_disassembled(offset, op, span, None)
                .expect("writing to string can't fail");
            disassembled
        } else {
            let mut skiped_iter = iter.skip(at - 1);
            let (_, (_, span)) = skiped_iter.next().expect("disassembling unknown index");
            let last_line_number = self.source.read_span(span, 0, 0).unwrap().line();
            let (offset, (op, span)) = skiped_iter
                .next()
                .expect("trying to disassemble empty chunk");
            let (disassembled, _) = self
                .to_disassembled(offset, op, span, Some(last_line_number))
                .expect("writing to string can't fail");
            disassembled
        }
    }

    pub fn line_number(&self, at: usize) -> usize {
        let location = self.locations[at];
        self.source.read_span(&location, 0, 0).unwrap().line() + 1
    }

    fn to_disassembled(
        &self,
        offset: usize,
        op: &Op,
        span: &SourceSpan,
        last_line_number: Option<usize>,
    ) -> Result<(String, usize), Error> {
        let mut result = String::new();
        let line_number = self.source.read_span(span, 0, 0).unwrap().line();

        write!(&mut result, "{offset:0>4} ")?;

        if last_line_number.is_some_and(|l| l == line_number) {
            write!(&mut result, "   | ")?;
        } else {
            write!(&mut result, "{:>4} ", line_number + 1)?;
        }

        match op {
            Op::Constant(idx)
            | Op::DefineGlobal(idx)
            | Op::GetGlobal(idx)
            | Op::SetGlobal(idx)
            | Op::Class(idx)
            | Op::Method(idx)
            | Op::GetProperty(idx)
            | Op::SetProperty(idx) => {
                let const_index: usize = (*idx).into();
                let constant = self.constants[const_index];
                write!(&mut result, "{:<16} {:<4} '{}'", op, const_index, constant)?;
            }
            Op::GetLocal(byte)
            | Op::SetLocal(byte)
            | Op::GetUpvalue(byte)
            | Op::SetUpvalue(byte)
            | Op::Call(byte) => write!(&mut result, "{:<16} {:<4}", op, byte)?,
            Op::JumpIfFalse(jump) | Op::Jump(jump) => write!(
                &mut result,
                "{:<16} {:0>4} -> {:0>4}",
                op,
                offset,
                offset + (*jump as usize)
            )?,
            Op::Loop(jump) => write!(
                &mut result,
                "{:<16} {:0>4} -> {:0>4}",
                op,
                offset,
                offset - (*jump as usize)
            )?,
            Op::Closure(idx) => {
                let const_index: usize = (*idx).into();
                let constant = self.constants[const_index];
                write!(&mut result, "{:<16} {:<4} '{}'", op, const_index, constant)?;
                if let Value::Obj(obj) = constant {
                    if let Obj::Function(function) = obj.deref() {
                        function.upvalues().iter().for_each(|u| {
                            write!(
                                &mut result,
                                "\n{} '{}'",
                                if u.is_local() { "local" } else { "upvalue" },
                                u.index()
                            )
                            .unwrap()
                        });
                    }
                }
            }
            Op::Invoke {
                property_index,
                arg_count,
            } => {
                let const_index: usize = (*property_index).into();
                let constant = self.constants[const_index];
                write!(
                    &mut result,
                    "{:<16} ({} args){:<4} '{}'",
                    op, arg_count, const_index, constant
                )?;
            }
            op => write!(&mut result, "{op}")?,
        }
        Ok((result, line_number))
    }
}

impl Debug for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chunk")
            .field("code", &self.code)
            .field("constants", &self.constants)
            .finish()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn constant_index() {
        let mut chunk: Chunk = Chunk::new(Arc::new(NamedSource::new("test", String::new())));
        let index = chunk.add_constant(Value::Number(12.1));
        assert_eq!(index, 0);
        let index = chunk.add_constant(Value::Number(12.1));
        assert_eq!(index, 1)
    }

    #[test]
    fn disassemble_constant() {
        let src = "1.1".to_string();
        let src = Arc::new(NamedSource::new("src", src));
        let mut chunk = Chunk::new(src);
        let constant = chunk.add_constant(Value::Number(1.1));
        chunk.write(Op::Constant(constant), SourceSpan::from((0, 3)));
        let res = chunk.disassemble_at(0);
        assert_eq!(res, "0000    1 CONSTANT         0    '1.1'");
    }
}
