use strum::Display;

#[derive(Debug, Clone, Copy, PartialEq, Display)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum Op {
    Constant(u8),
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
    Nil,
    True,
    False,
    Not,
    Equal,
    Greater,
    Less,
    Print,
    Pop,
    DefineGlobal(u8),
    GetGlobal(u8),
    SetGlobal(u8),
}
