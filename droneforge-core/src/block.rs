use serde::{Deserialize, Serialize};

pub type BlockId = u16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub id: BlockId,
    pub name: String,
}

impl Block {
    pub fn new(id: BlockId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }
}
