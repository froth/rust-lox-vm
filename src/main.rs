use chunk::Chunk;

mod chunk;
mod memory;
fn main() {
    let mut chunk = Chunk::new();
    chunk.write_chunk(chunk::OpCode::Return);
    chunk.write_chunk(chunk::OpCode::Constant);
    println!(
        "head: {:?}, cap: {}, count: {}",
        chunk[0],
        chunk.capacity(),
        chunk.len()
    );
    chunk.clear();
    chunk.write_chunk(chunk::OpCode::Constant);
}
