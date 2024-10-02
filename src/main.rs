use chunk::Chunk;
use miette::{NamedSource, Result, SourceSpan};
use op::Op;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use vm::VM;

mod chunk;
mod lox_vector;
mod memory;
mod op;
mod value;
mod vm;

fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let src = "1.1;return";
    let src = NamedSource::new("src", src);
    let mut chunk = Chunk::new();
    let constant = chunk.add_constant(1.1);
    chunk.write(Op::Constant(constant), SourceSpan::from((0, 3)));
    chunk.write(Op::Constant(constant), SourceSpan::from((0, 3)));
    chunk.write(Op::Constant(constant), SourceSpan::from((0, 3)));
    chunk.write(Op::Constant(constant), SourceSpan::from((0, 3)));
    chunk.write(Op::Constant(constant), SourceSpan::from((0, 3)));
    chunk.write(Op::Return, SourceSpan::from((5, 6)));
    chunk.disassemble(&src);
    let mut vm = VM::new();
    vm.interpret(chunk, &src)
}
