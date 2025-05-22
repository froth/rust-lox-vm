use std::fmt::Display;

use super::{Hash, Hashable};

#[derive(Debug, Clone, PartialEq)]
pub struct LoxString {
    pub string: String,
    hash: Hash,
}

impl LoxString {
    pub fn from_str(s: &str) -> Self {
        Self::string(s.to_owned())
    }

    pub fn string(string: String) -> Self {
        let hash = hash_str(&string);
        Self { string, hash }
    }
}

pub fn hash_str(str: &str) -> Hash {
    const PRIME: u32 = 16777619;
    let mut hash: u32 = 2166136261;
    for b in str.bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(PRIME);
    }
    Hash(hash)
}

impl Hashable for LoxString {
    fn hash(&self) -> Hash {
        self.hash
    }
}

impl Display for LoxString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.string)
    }
}
