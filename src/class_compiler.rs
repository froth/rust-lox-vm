pub struct ClassCompiler {
    pub enclosing: Option<Box<ClassCompiler>>,
    pub has_superclass: bool,
}
impl ClassCompiler {
    pub(crate) fn new() -> Self {
        Self {
            enclosing: None,
            has_superclass: false,
        }
    }
}
