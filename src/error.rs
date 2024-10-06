use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum InterpreterError {
    #[error("Oops compiler blew up")]
    CompileError,
    #[error("Oops vm blew up")]
    RuntimeError,
}
