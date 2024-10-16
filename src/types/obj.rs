use std::fmt::Display;

#[derive(Debug, Clone, PartialEq)]
pub enum Obj {
    String(LoxString),
    Class { name: LoxString },
}

impl Obj {
    pub fn from_str(s: &str) -> Self {
        Self::String(LoxString::string(s.to_owned()))
    }

    pub fn string(string: String) -> Self {
        Self::String(LoxString::string(string))
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Obj::String(s) => write!(f, "\"{}\"", s.string),
            Obj::Class { name: _ } => todo!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoxString {
    pub string: String,
    pub hash: u32,
}

impl LoxString {
    pub fn from_str(s: &str) -> Self {
        Self::string(s.to_owned())
    }

    pub fn string(string: String) -> Self {
        const PRIME: u32 = 16777619;
        let mut hash: u32 = 2166136261;
        for b in string.bytes() {
            hash ^= b as u32;
            hash = hash.wrapping_mul(PRIME);
        }
        Self { string, hash }
    }
}
