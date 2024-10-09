use miette::{Diagnostic, Report};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum InterpreterError {
    #[error("Oops compiler blew up")]
    CompileError(Report),
    #[error("Oops vm blew up")]
    RuntimeError(Report),
}
