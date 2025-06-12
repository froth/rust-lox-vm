use miette::{Diagnostic, Report};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum InterpreterError {
    #[diagnostic(transparent)]
    #[error("Parser Error")]
    CompileError(Report),
    #[error("Runtime Error")]
    RuntimeError {
        #[diagnostic_source]
        error: Report,
        stacktrace: String,
    },
}
