use miette::{LabeledSpan, Result, SourceSpan};

use crate::{chunk::Chunk, types::function::Function};
#[derive(PartialEq, Debug)]
struct Local<'a> {
    name: &'a str,
    depth: Option<u32>,
}

pub enum FunctionType {
    Function,
    Script,
}

pub struct Compiler<'a> {
    function_type: FunctionType,
    locals: Vec<Local<'a>>,
    scope_depth: u32,
    pub chunk: Chunk,
}

#[derive(PartialEq, Debug)]
pub struct ResolveResult {
    pub slot: usize,
    pub initialized: bool,
}

impl<'a> Compiler<'a> {
    pub fn new(function_type: FunctionType) -> Self {
        let slot_zero = Local {
            name: "",
            depth: Some(0),
        };
        Self {
            locals: vec![slot_zero],
            scope_depth: 0,
            function_type,
            chunk: Chunk::new(),
        }
    }

    pub fn is_local(&self) -> bool {
        self.scope_depth > 0
    }

    pub fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    pub fn end_scope(&mut self) -> usize {
        self.scope_depth -= 1;

        let mut popped: usize = 0;
        while let Some(last) = self.locals.last() {
            if last.depth.is_none_or(|s| s > self.scope_depth) {
                self.locals.pop();
                popped += 1;
            } else {
                break;
            }
        }
        popped
    }

    pub fn add_local(&mut self, name: &'a str, location: SourceSpan) -> Result<()> {
        if self.locals.len() > u8::MAX as usize {
            miette::bail!(
                labels = vec![LabeledSpan::at(location, "here")],
                "Too many local variables in function.",
            )
        }
        let local = Local { name, depth: None };
        self.locals.push(local);
        Ok(())
    }

    pub fn mark_latest_initialized(&mut self) {
        if let Some(last) = self.locals.last_mut() {
            last.depth = Some(self.scope_depth);
        }
    }

    pub fn has_variable_in_current_scope(&self, name: &str) -> bool {
        self.locals
            .iter()
            .rev()
            .take_while(|l| l.depth.is_none_or(|d| d == self.scope_depth))
            .any(|l| l.name == name)
    }

    pub fn resolve_locale(&self, name: &str) -> Option<ResolveResult> {
        self.locals
            .iter()
            .enumerate()
            .rev()
            .find(|(_, l)| l.name == name)
            .map(|(position, l)| ResolveResult {
                slot: position,
                initialized: l.depth.is_some(),
            })
    }

    pub fn end_compiler(self) -> Function {
        Function::new(0, self.chunk, None) // TODO: Real names for real functions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_works() {
        let compiler = Compiler::new(FunctionType::Script);
        assert_eq!(
            compiler.locals,
            vec![Local {
                name: "",
                depth: Some(0)
            }]
        ); // slot zero
        assert_eq!(compiler.scope_depth, 0);
    }

    #[test]
    fn has_variable_on_empty() {
        let compiler = Compiler::new(FunctionType::Script);
        assert!(!compiler.has_variable_in_current_scope("asdasd"));
    }

    #[test]
    fn has_variable_on_upper_scope() {
        let compiler = Compiler {
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(1),
                },
                Local {
                    name: "b",
                    depth: Some(2),
                },
            ],
            scope_depth: 2,
            function_type: FunctionType::Script,
            chunk: Chunk::new(),
        };
        assert!(!compiler.has_variable_in_current_scope("a"));
    }

    #[test]
    fn has_variable_on_current_scope() {
        let compiler = Compiler {
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(2),
                },
                Local {
                    name: "b",
                    depth: Some(2),
                },
            ],
            scope_depth: 2,
            function_type: FunctionType::Script,
            chunk: Chunk::new(),
        };
        assert!(compiler.has_variable_in_current_scope("a"));
    }

    #[test]
    fn has_variable_on_current_scope_with_uninitialized_behind() {
        let compiler = Compiler {
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(1),
                },
                Local {
                    name: "b",
                    depth: Some(2),
                },
                Local {
                    name: "c",
                    depth: None,
                },
            ],
            scope_depth: 2,
            function_type: FunctionType::Script,
            chunk: Chunk::new(),
        };
        assert!(compiler.has_variable_in_current_scope("b"));
    }

    #[test]
    fn end_scope_returns_correct_count() {
        let mut compiler = Compiler {
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(1),
                },
                Local {
                    name: "b",
                    depth: Some(2),
                },
                Local {
                    name: "c",
                    depth: None,
                },
            ],
            scope_depth: 2,
            function_type: FunctionType::Script,
            chunk: Chunk::new(),
        };
        assert_eq!(compiler.end_scope(), 2);
    }

    #[test]
    fn resolve_local_uninitialized() {
        let compiler = Compiler {
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(1),
                },
                Local {
                    name: "a",
                    depth: Some(2),
                },
                Local {
                    name: "a",
                    depth: None,
                },
            ],
            scope_depth: 2,
            function_type: FunctionType::Script,
            chunk: Chunk::new(),
        };
        assert_eq!(
            compiler.resolve_locale("a"),
            Some(ResolveResult {
                slot: 2,
                initialized: false
            })
        );
    }

    #[test]
    fn resolve_local_initialized() {
        let compiler = Compiler {
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(1),
                },
                Local {
                    name: "a",
                    depth: Some(2),
                },
                Local {
                    name: "b",
                    depth: None,
                },
            ],
            scope_depth: 2,
            function_type: FunctionType::Script,
            chunk: Chunk::new(),
        };
        assert_eq!(
            compiler.resolve_locale("a"),
            Some(ResolveResult {
                slot: 1,
                initialized: true
            })
        );
    }
}
