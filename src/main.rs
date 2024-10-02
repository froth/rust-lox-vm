use chunk::Chunk;
use miette::{NamedSource, SourceSpan};
use op::Op;
use tracing::{debug, Level};
use tracing_subscriber::FmtSubscriber;

mod chunk;
mod lox_vector;
mod memory;
mod op;
mod value;
fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let src = "return 1.1;
return 1.1;
return;
    ";
    let src = NamedSource::new("src", src);
    let mut chunk = Chunk::new();
    let _ = chunk.add_constant(0.0);
    let _ = chunk.add_constant(0.0);
    let _ = chunk.add_constant(0.0);
    let _ = chunk.add_constant(0.0);
    let _ = chunk.add_constant(0.0);
    chunk.write(Op::Return, SourceSpan::from((0, 6)));
    let constant = chunk.add_constant(1.1);
    chunk.write(Op::Constant(constant), SourceSpan::from((7, 3)));
    chunk.write(Op::Return, SourceSpan::from((12, 6)));
    let constant = chunk.add_constant(1.1);
    chunk.write(Op::Constant(constant), SourceSpan::from((19, 3)));
    chunk.write(Op::Return, SourceSpan::from((24, 6)));
    chunk.disassemble(&src);

    debug!("{}", chunk.disassemble_at(&src, 0));
    debug!("{}", chunk.disassemble_at(&src, 1));
    debug!("{}", chunk.disassemble_at(&src, 2));
    debug!("{}", chunk.disassemble_at(&src, 4));
}
