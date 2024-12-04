use miette::{Diagnostic, Report};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum InterpreterError {
    #[diagnostic(transparent)]
    #[error("Parser Error")]
    CompileError(Report),
    #[diagnostic(transparent)]
    #[error("Oops vm blew up")]
    RuntimeError(Report),
}
