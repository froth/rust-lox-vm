#[derive(PartialEq, Debug)]
pub struct UpvalueIndex {
    index: u8,
    is_local: bool,
}
impl UpvalueIndex {
    pub fn new(index: u8, is_local: bool) -> Self {
        Self { index, is_local }
    }

    pub fn index(&self) -> u8 {
        self.index
    }

    pub fn is_local(&self) -> bool {
        self.is_local
    }
}
