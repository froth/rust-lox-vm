use args::Args;
use clap::Parser as _;
use error::InterpreterError;
use lox::Lox;
use miette::{IntoDiagnostic, NamedSource, Result};
use rustyline::{
    error::ReadlineError, highlight::MatchingBracketHighlighter,
    validate::MatchingBracketValidator, Completer, Editor, Helper, Highlighter, Hinter, Validator,
};
use std::fs;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

mod args;
mod chunk;
mod error;
mod lox;
mod lox_vector;
mod memory;
mod op;
mod scanner;
mod token;
mod value;
mod vm;

fn main() {
    let args = Args::parse();
    let level = if args.verbose {
        Level::TRACE
    } else {
        Level::INFO
    };
    let subscriber = FmtSubscriber::builder().with_max_level(level).finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();
    let lox = Lox::new();
    let result = match args.file {
        Some(file) => run_file(lox, file),
        None => run_prompt(lox, args).into_diagnostic(),
    };
    match result {
        Ok(_) => (),
        Err(err) => {
            eprintln!("{:?}", err);
            let return_code = if let Some(compile_error) = err.downcast_ref::<InterpreterError>() {
                match compile_error {
                    InterpreterError::CompileError => 65,
                    InterpreterError::RuntimeError => 70,
                }
            } else {
                74
            };
            std::process::exit(return_code)
        }
    };
}

fn run_file(mut lox: Lox, file: String) -> Result<()> {
    let contents = fs::read_to_string(file.clone()).into_diagnostic()?;

    let named_source = NamedSource::new(file, contents);
    lox.run(named_source)
}

fn run_prompt(mut lox: Lox, args: Args) -> rustyline::Result<()> {
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
                match lox.run_repl(NamedSource::new("repl", source)) {
                    Ok(Some(value)) => println!("expr => {}", value),
                    Ok(None) => (),
                    Err(err) => println!("{:?}", err),
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
