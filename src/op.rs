#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Return,
    Constant(u8),
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
}
