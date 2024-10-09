use miette::Report;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InterpreterError {
    #[error("Oops compiler blew up")]
    CompileError(Report),
    #[error("Oops vm blew up")]
    RuntimeError(Report),
}
