use serde::{Deserialize, Serialize};

pub type BlockId = u16;

pub const AIR: BlockId = 0;
pub const DIRT: BlockId = 1;
pub const STONE: BlockId = 2;
pub const IRON: BlockId = 3;
pub const BEDROCK: BlockId = 4;

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
