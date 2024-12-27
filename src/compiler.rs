struct Local<'a> {
    name: &'a str,
    depth: Option<u32>,
}

pub struct Compiler<'a> {
    locals: Vec<Local<'a>>,
    scope_depth: u32,
}

impl Compiler<'_> {
    pub fn new() -> Self {
        Self {
            locals: vec![],
            scope_depth: 0,
        }
    }

    pub fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    pub fn end_scope(&mut self) {
        self.scope_depth -= 1;
        //TODO: return number of pops
    }
}
