use chunk::Chunk;
use miette::{NamedSource, SourceSpan};

mod chunk;
mod lox_vector;
mod memory;
mod value;
fn main() {
    let src = "return 1.1;
return 1.1;
return;
    ";
    let src = NamedSource::new("src", src);
    let mut chunk = Chunk::new();
    chunk.write_op_code(chunk::OpCode::Return, SourceSpan::from((0, 6)));
    let constant = chunk.add_constant(1.1);
    chunk.write_op_code(chunk::OpCode::Constant, SourceSpan::from((7, 3)));
    chunk.write(constant, SourceSpan::from((7, 3)));
    chunk.write_op_code(chunk::OpCode::Return, SourceSpan::from((12, 6)));
    let constant = chunk.add_constant(1.1);
    chunk.write_op_code(chunk::OpCode::Constant, SourceSpan::from((19, 3)));
    chunk.write(constant, SourceSpan::from((19, 3)));
    chunk.write_op_code(chunk::OpCode::Return, SourceSpan::from((24, 6)));
    chunk.disassemble(src);
}
