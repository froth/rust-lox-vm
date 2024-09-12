use chunk::Chunk;

mod chunk;
mod memory;
fn main() {
    let mut chunk = Chunk::new();
    chunk.write_chunk(chunk::OpCode::Return);
    chunk.write_chunk(chunk::OpCode::Constant);
    println!(
        "head: {:?}, cap: {}, count: {}",
        chunk.head(),
        chunk.capacity(),
        chunk.count()
    )
}
