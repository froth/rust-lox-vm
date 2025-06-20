use std::sync::Arc;

use miette::{LabeledSpan, NamedSource, Result, SourceSpan};

use crate::{
    chunk::Chunk,
    op::Op,
    types::{obj_ref::ObjRef, upvalue::UpvalueIndex, value::Value},
};
#[derive(PartialEq, Debug)]
struct Local<'a> {
    name: &'a str,
    depth: Option<u32>,
    is_captured: bool,
}

#[derive(PartialEq, Debug)]
pub enum FunctionType {
    Function,
    Script,
    Method,
    Initializer,
}

pub struct Compiler<'a> {
    pub enclosing: Option<Box<Compiler<'a>>>,
    pub function_type: FunctionType,
    pub function_name: Option<String>,
    pub arity: u8,
    locals: Vec<Local<'a>>,
    pub upvalues: Vec<UpvalueIndex>,
    scope_depth: u32,
    pub chunk: Chunk,
}

#[derive(PartialEq, Debug)]
pub struct ResolveResult {
    pub slot: u8,
    pub initialized: bool,
}
pub struct Jump {
    op: fn(u16) -> Op,
    location: SourceSpan,
    position: usize,
}

impl<'a> Compiler<'a> {
    pub fn new(
        function_type: FunctionType,
        function_name: Option<String>,
        src: Arc<NamedSource<String>>,
    ) -> Self {
        let slot_zero_name = if matches!(
            function_type,
            FunctionType::Method | FunctionType::Initializer
        ) {
            "this"
        } else {
            ""
        };
        let slot_zero = Local {
            name: slot_zero_name,
            depth: Some(0),
            is_captured: false,
        };
        Self {
            enclosing: None,
            function_type,
            function_name,
            arity: 0,
            locals: vec![slot_zero],
            upvalues: vec![],
            scope_depth: 0,
            chunk: Chunk::new(src),
        }
    }

    pub fn is_local(&self) -> bool {
        self.scope_depth > 0
    }

    pub fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    pub fn end_scope(&mut self, location: SourceSpan) {
        self.scope_depth -= 1;

        while let Some(last) = self.locals.last() {
            if last.depth.is_none_or(|s| s > self.scope_depth) {
                if last.is_captured {
                    self.chunk.write(Op::CloseUpvalue, location);
                } else {
                    self.chunk.write(Op::Pop, location);
                }
                self.locals.pop();
            } else {
                break;
            }
        }
    }

    pub fn add_local(&mut self, name: &'a str, location: SourceSpan) -> Result<()> {
        if self.locals.len() > u8::MAX as usize {
            miette::bail!(
                labels = vec![LabeledSpan::at(location, "here")],
                "Too many local variables in function.",
            )
        }
        let local = Local {
            name,
            depth: None,
            is_captured: false,
        };
        self.locals.push(local);
        Ok(())
    }

    pub fn mark_latest_initialized(&mut self) {
        if self.scope_depth > 0 {
            // happens in global function declaration
            if let Some(last) = self.locals.last_mut() {
                last.depth = Some(self.scope_depth);
            }
        }
    }

    pub fn has_variable_in_current_scope(&self, name: &str) -> bool {
        self.locals
            .iter()
            .rev()
            .take_while(|l| l.depth.is_none_or(|d| d == self.scope_depth))
            .any(|l| l.name == name)
    }

    pub fn resolve_local(&self, name: &str) -> Option<ResolveResult> {
        self.locals
            .iter()
            .enumerate()
            .rev()
            .find(|(_, l)| l.name == name)
            .map(|(position, l)| ResolveResult {
                slot: position as u8,
                initialized: l.depth.is_some(),
            })
    }

    pub fn resolve_upvalue(&mut self, name: &str) -> Option<u8> {
        if let Some(enclosing) = self.enclosing.as_mut() {
            if let Some(local) = enclosing.resolve_local(name) {
                enclosing.locals[local.slot as usize].is_captured = true;
                return Some(self.add_upvalue(local.slot, true));
            } else if let Some(non_local) = enclosing.resolve_upvalue(name) {
                return Some(self.add_upvalue(non_local, false));
            }
        }
        None
    }

    fn add_upvalue(&mut self, index: u8, is_local: bool) -> u8 {
        let upvalue = UpvalueIndex::new(index, is_local);
        if let Some(i) = self.upvalues.iter().position(|u| u == &upvalue) {
            i as u8
        } else {
            assert!(self.upvalues.len() <= u8::MAX as usize, "too many upvalues");
            self.upvalues.push(UpvalueIndex::new(index, is_local));
            (self.upvalues.len() - 1) as u8
        }
    }

    pub fn define_variable(&mut self, global_idx: Option<u8>, location: SourceSpan) {
        if let Some(const_idx) = global_idx {
            self.chunk.write(Op::DefineGlobal(const_idx), location);
        } else {
            self.mark_latest_initialized();
        }
    }

    pub fn declare_variable(&mut self, name: &'a str, location: SourceSpan) -> Result<()> {
        if self.is_local() {
            if self.has_variable_in_current_scope(name) {
                miette::bail!(
                    labels = vec![LabeledSpan::at(location, "here")],
                    "Already a variable with this name in this scope"
                )
            }
            self.add_local(name, location)?;
        }
        Ok(())
    }

    pub fn identifier_constant(&mut self, name: ObjRef) -> u8 {
        self.chunk.add_constant(Value::Obj(name))
    }

    pub fn emit_constant(&mut self, value: Value, location: SourceSpan) {
        let idx = self.chunk.add_constant(value);
        self.chunk.write(Op::Constant(idx), location);
    }

    pub fn emit_jump(&mut self, op: fn(u16) -> Op, location: SourceSpan) -> Jump {
        let position = self.chunk.code.len();
        self.chunk.write(op(0), location);
        Jump {
            op,
            location,
            position,
        }
    }

    pub fn emit_loop(&mut self, loop_start: usize, location: SourceSpan) -> Result<()> {
        let jump_length = self.chunk.code.len() - loop_start;
        if let Ok(jump_length) = u16::try_from(jump_length) {
            self.chunk.write(Op::Loop(jump_length), location);
            Ok(())
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(location, "here")],
                "Loop body too large."
            )
        }
    }

    pub fn patch_jump(&mut self, jump: Jump) -> Result<()> {
        let jump_length = self.chunk.code.len() - jump.position;
        if let Ok(jump_length) = u16::try_from(jump_length) {
            self.chunk.code[jump.position] = (jump.op)(jump_length);
            Ok(())
        } else {
            miette::bail!(
                labels = vec![LabeledSpan::at(jump.location, "here")],
                "Too much code to jump over"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    fn empty_src() -> Arc<NamedSource<String>> {
        Arc::new(NamedSource::new("name", String::new()))
    }
    #[test]
    fn new_works() {
        let compiler = Compiler::new(FunctionType::Script, None, empty_src());
        assert_eq!(
            compiler.locals,
            vec![Local {
                name: "",
                depth: Some(0),
                is_captured: false
            }]
        ); // slot zero
        assert_eq!(compiler.scope_depth, 0);
    }

    #[test]
    fn has_variable_on_empty() {
        let compiler = Compiler::new(FunctionType::Script, None, empty_src());
        assert!(!compiler.has_variable_in_current_scope("asdasd"));
    }

    #[test]
    fn has_variable_on_upper_scope() {
        let compiler = Compiler {
            enclosing: None,
            arity: 0,
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(1),
                    is_captured: false,
                },
                Local {
                    name: "b",
                    depth: Some(2),
                    is_captured: false,
                },
            ],
            upvalues: vec![],
            scope_depth: 2,
            function_type: FunctionType::Script,
            function_name: None,
            chunk: Chunk::new(empty_src()),
        };
        assert!(!compiler.has_variable_in_current_scope("a"));
    }

    #[test]
    fn has_variable_on_current_scope() {
        let compiler = Compiler {
            enclosing: None,
            arity: 0,
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(2),
                    is_captured: false,
                },
                Local {
                    name: "b",
                    depth: Some(2),
                    is_captured: false,
                },
            ],
            upvalues: vec![],
            scope_depth: 2,
            function_type: FunctionType::Script,
            function_name: None,
            chunk: Chunk::new(empty_src()),
        };
        assert!(compiler.has_variable_in_current_scope("a"));
    }

    #[test]
    fn has_variable_on_current_scope_with_uninitialized_behind() {
        let compiler = Compiler {
            enclosing: None,
            arity: 0,
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(1),
                    is_captured: false,
                },
                Local {
                    name: "b",
                    depth: Some(2),
                    is_captured: false,
                },
                Local {
                    name: "c",
                    depth: None,
                    is_captured: false,
                },
            ],
            upvalues: vec![],
            scope_depth: 2,
            function_type: FunctionType::Script,
            function_name: None,
            chunk: Chunk::new(empty_src()),
        };
        assert!(compiler.has_variable_in_current_scope("b"));
    }

    #[test]
    fn end_scope_writes_enough_pops() {
        let location = SourceSpan::from((0, 0));
        let mut compiler = Compiler {
            enclosing: None,
            arity: 0,
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(1),
                    is_captured: false,
                },
                Local {
                    name: "b",
                    depth: Some(2),
                    is_captured: false,
                },
                Local {
                    name: "c",
                    depth: None,
                    is_captured: false,
                },
            ],
            upvalues: vec![],
            scope_depth: 2,
            function_type: FunctionType::Script,
            function_name: None,
            chunk: Chunk::new(empty_src()),
        };
        compiler.end_scope(location);
        assert_eq!(compiler.chunk.code.len(), 2);
        assert_eq!(compiler.chunk.code[0], Op::Pop);
        assert_eq!(compiler.chunk.code[1], Op::Pop);
    }

    #[test]
    fn resolve_local_uninitialized() {
        let compiler = Compiler {
            enclosing: None,
            arity: 0,
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(1),
                    is_captured: false,
                },
                Local {
                    name: "a",
                    depth: Some(2),
                    is_captured: false,
                },
                Local {
                    name: "a",
                    depth: None,
                    is_captured: false,
                },
            ],
            upvalues: vec![],
            scope_depth: 2,
            function_type: FunctionType::Script,
            function_name: None,
            chunk: Chunk::new(empty_src()),
        };
        assert_eq!(
            compiler.resolve_local("a"),
            Some(ResolveResult {
                slot: 2,
                initialized: false
            })
        );
    }

    #[test]
    fn resolve_local_initialized() {
        let compiler = Compiler {
            enclosing: None,
            arity: 0,
            locals: vec![
                Local {
                    name: "a",
                    depth: Some(1),
                    is_captured: false,
                },
                Local {
                    name: "a",
                    depth: Some(2),
                    is_captured: false,
                },
                Local {
                    name: "b",
                    depth: None,
                    is_captured: false,
                },
            ],
            upvalues: vec![],
            scope_depth: 2,
            function_type: FunctionType::Script,
            function_name: None,
            chunk: Chunk::new(empty_src()),
        };
        assert_eq!(
            compiler.resolve_local("a"),
            Some(ResolveResult {
                slot: 1,
                initialized: true
            })
        );
    }

    #[test]
    fn add_upvalue_deduplicates() {
        let mut compiler = Compiler::new(FunctionType::Function, None, empty_src());
        compiler.add_upvalue(4, true);
        compiler.add_upvalue(2, false);
        compiler.add_upvalue(4, true);
        assert_eq!(
            compiler.upvalues,
            vec![UpvalueIndex::new(4, true), UpvalueIndex::new(2, false)]
        );
    }
}
