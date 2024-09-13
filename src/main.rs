use chunk::Chunk;

mod chunk;
mod lox_vector;
mod memory;
mod value;
fn main() {
    let mut chunk = Chunk::new();
    chunk.write_op_code(chunk::OpCode::Return);
    let constant = chunk.add_constant(1.1);
    chunk.write_op_code(chunk::OpCode::Constant);
    chunk.write(constant);
    let constant = chunk.add_constant(1.1);
    chunk.write_op_code(chunk::OpCode::Constant);
    chunk.write(constant);
    chunk.write_op_code(chunk::OpCode::Return);
    chunk.write_op_code(chunk::OpCode::Return);
    chunk.write_op_code(chunk::OpCode::Return);
    chunk.write_op_code(chunk::OpCode::Return);
    chunk.disassemble("foo");
}
