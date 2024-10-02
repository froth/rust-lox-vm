#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Return,
    Constant(u8),
}
