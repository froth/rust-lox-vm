use chunk::Chunk;

mod chunk;
mod lox_vector;
mod memory;
fn main() {
    let mut chunk = Chunk::new();
    chunk.write_chunk(chunk::OpCode::Return);
    chunk.write_chunk(chunk::OpCode::Constant);
    chunk.write_chunk(chunk::OpCode::Constant);
    chunk.disassemble("foo");
}
