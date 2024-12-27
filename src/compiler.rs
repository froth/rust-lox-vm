use miette::{ByteOffset, Diagnostic, LabeledSpan, NamedSource, Report, Result, SourceSpan};
struct Local<'a> {
    name: &'a str,
    depth: Option<u32>,
}

pub struct Compiler<'a> {
    locals: Vec<Local<'a>>,
    scope_depth: u32,
}

impl<'a> Compiler<'a> {
    pub fn new() -> Self {
        Self {
            locals: vec![],
            scope_depth: 0,
        }
    }

    pub fn is_local(&self) -> bool {
        self.scope_depth > 0
    }
    pub fn is_global(&self) -> bool {
        self.scope_depth == 0
    }

    pub fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    pub fn end_scope(&mut self) {
        self.scope_depth -= 1;
        //TODO: return number of pops
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
            depth: Some(self.scope_depth),
        };
        self.locals.push(local);
        Ok(())
    }
}
