pub mod class;
pub mod function;
pub mod instance;
pub mod obj;
pub mod obj_ref;
pub mod string;
pub mod upvalue;
pub mod value;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Hash(pub u32);

pub trait Hashable {
    fn hash(&self) -> Hash;
}
