pub struct ClassCompiler {
    pub enclosing: Option<Box<ClassCompiler>>,
}
impl ClassCompiler {
    pub(crate) fn new() -> Self {
        Self { enclosing: None }
    }
}
