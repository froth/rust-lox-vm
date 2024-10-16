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
        const PRIME: u32 = 16777619;
        let mut hash: u32 = 2166136261;
        for b in string.bytes() {
            hash ^= b as u32;
            hash = hash.wrapping_mul(PRIME);
        }
        Self {
            string,
            hash: Hash(hash),
        }
    }
}

impl Hashable for LoxString {
    fn hash(&self) -> Hash {
        self.hash
    }
}
