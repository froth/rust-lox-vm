use miette::{Diagnostic, Report};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum InterpreterError {
    #[diagnostic(transparent)]
    #[error("Parser Error")]
    CompileError(Report),
    #[diagnostic()]
    #[error("Oops vm blew up")]
    RuntimeError { error: Report, stacktrace: String },
}
