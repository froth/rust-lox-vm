use strum::Display;

#[derive(Debug, Clone, Copy, PartialEq, Display)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum Op {
    Return,
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
    GetLocal(u8),
    SetLocal(u8),
    GetUpvalue(u8),
    SetUpvalue(u8),
    GetProperty(u8),
    SetProperty(u8),
    JumpIfFalse(u16), // TODO: read op_codes and parameters separately to decreace opcode size? As in clox
    Jump(u16),
    Loop(u16),
    Call(u8),
    Closure(u8),
    CloseUpvalue,
    Class(u8),
}
