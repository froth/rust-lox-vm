use miette::{NamedSource, Result};

use crate::{scanner::Scanner, value::Value};

pub struct Lox;

impl Lox {
    pub fn new() -> Self {
        Lox {}
    }

    pub fn run(&mut self, src: NamedSource<String>) -> Result<()> {
        let scanner = Scanner::new(&src);
        for a in scanner {
            let token = a?;
            println!("{:?}", token.token_type)
        }
        Ok(())
    }

    pub fn run_repl(&mut self, src: NamedSource<String>) -> Result<Option<Value>> {
        let scanner = Scanner::new(&src);
        for a in scanner {
            let token = a?;
            println!("{:?}", token.token_type)
        }
        Ok(None)
    }
}
