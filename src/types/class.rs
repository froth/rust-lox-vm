use crate::types::string::LoxString;
use std::fmt::Display;

#[derive(Debug)]
pub struct Class {
    name: LoxString,
}

impl Class {
    pub fn new(name: LoxString) -> Self {
        Self { name }
    }

    pub fn name(&self) -> &LoxString {
        &self.name
    }
}
impl Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
