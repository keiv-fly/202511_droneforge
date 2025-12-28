use serde::{Deserialize, Serialize};

pub type BlockId = u16;

pub const AIR: BlockId = 0;
pub const DIRT: BlockId = 1;
pub const STONE: BlockId = 2;
pub const IRON: BlockId = 3;
pub const BEDROCK: BlockId = 4;
pub const CORE: BlockId = 5;

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

pub fn is_placable_block(block: BlockId) -> bool {
    block == CORE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_block_with_provided_fields() {
        let block = Block::new(DIRT, "Dirt");

        assert_eq!(block.id, DIRT);
        assert_eq!(block.name, "Dirt");
    }

    #[test]
    fn serde_round_trip_preserves_block() {
        let original = Block::new(IRON, "Iron");

        let serialized = serde_json::to_string(&original).expect("serialization should succeed");
        let restored: Block =
            serde_json::from_str(&serialized).expect("deserialization should succeed");

        assert_eq!(restored, original);
    }

    #[test]
    fn placable_block_flags_core_only() {
        assert!(is_placable_block(CORE));
        assert!(!is_placable_block(AIR));
        assert!(!is_placable_block(STONE));
    }
}
