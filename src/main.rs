use args::Args;
use clap::Parser as _;
use error::InterpreterError;
use miette::{IntoDiagnostic, NamedSource, Result};
use rustyline::{
    error::ReadlineError, highlight::MatchingBracketHighlighter,
    validate::MatchingBracketValidator, Completer, Editor, Helper, Highlighter, Hinter, Validator,
};
use std::fs;
use tracing::Level;
use tracing_subscriber::{filter, layer::SubscriberExt as _, util::SubscriberInitExt as _};
use vm::VM;

mod args;
mod chunk;
mod compiler;
mod datastructures;
mod error;
mod gc;
mod op;
mod parser;
mod printer;
mod scanner;
mod source_span_extensions;
mod token;
mod types;
mod vm;

fn main() {
    let args = Args::parse();
    let level = if args.verbose {
        Level::TRACE
    } else {
        Level::INFO
    };

    let gc_level = if args.gc_log {
        Level::TRACE
    } else {
        Level::INFO
    };

    let filter = filter::Targets::new()
        .with_target("rust_lox_vm::gc", gc_level)
        .with_target("rust_lox_vm::vm::gc", gc_level)
        .with_target("rust_lox_vm", level);

    // Build a new subscriber with the `fmt` layer using the `Targets`
    // filter we constructed above.
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    let vm = VM::new();
    let result = match args.file {
        Some(file) => run_file(vm, file),
        None => run_prompt(vm, args).into_diagnostic(),
    };
    match result {
        Ok(_) => (),
        Err(err) => {
            if let Some(compile_error) = err.downcast_ref::<InterpreterError>() {
                match compile_error {
                    InterpreterError::CompileError(err) => {
                        eprintln!("{:?}", err);
                        std::process::exit(65)
                    }
                    InterpreterError::RuntimeError { error, stacktrace } => {
                        eprintln!("{:?}", error);
                        eprintln!("{}", stacktrace);
                        std::process::exit(75)
                    }
                }
            } else {
                eprintln!("{:?}", err);
                std::process::exit(74)
            };
        }
    };
}

fn run_file(mut vm: VM, file: String) -> Result<()> {
    let contents = fs::read_to_string(file.clone()).into_diagnostic()?;

    let named_source = NamedSource::new(file, contents);
    vm.interpret(named_source)?;
    Ok(())
}

fn run_prompt(mut vm: VM, args: Args) -> rustyline::Result<()> {
    #[derive(Helper, Completer, Hinter, Validator, Highlighter, Default)]
    struct MyHelper {
        #[rustyline(Validator)]
        validator: MatchingBracketValidator,
        #[rustyline(Highlighter)]
        highlighter: MatchingBracketHighlighter,
    }

    let history_file = args.history_file;
    let mut rl = Editor::new()?;
    rl.set_helper(Some(MyHelper::default()));
    if let Err(err) = rl.load_history(&history_file).into_diagnostic() {
        eprintln!("No previous history: {:?}", &history_file);
        if args.verbose {
            eprintln!("Error: {:?}", err)
        }
    }
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(source) => {
                rl.add_history_entry(source.as_str())?;
                match vm.interpret(NamedSource::new("repl", source)) {
                    Ok(()) => (),
                    Err(InterpreterError::CompileError(err)) => println!("{:?}", err),
                    Err(InterpreterError::RuntimeError { error, stacktrace }) => {
                        println!("{:?}", error);
                        println!("{}", stacktrace);
                    }
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            err => {
                err?;
            }
        }
    }

    if let Err(err) = rl.save_history(&history_file).into_diagnostic() {
        eprintln!("Unable to save history: {:?}", err);
    }
    Ok(())
}

#[cfg(test)]
#[macro_use]
extern crate assert_matches;
