use crate::block::BlockId;
use crate::chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH};
use crate::coordinates::ChunkPosition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageError(pub String);

pub type SaveBlocksFn =
    Box<dyn Fn(ChunkPosition, Vec<BlockId>) -> Result<(), StorageError> + Send + Sync + 'static>;
pub type LoadBlocksFn = Box<
    dyn Fn(ChunkPosition) -> Result<Option<Vec<BlockId>>, StorageError> + Send + Sync + 'static,
>;

impl StorageError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

pub fn expected_block_count(width: usize, depth: usize, height: usize) -> usize {
    width * depth * height
}

pub fn default_block_count() -> usize {
    expected_block_count(CHUNK_WIDTH, CHUNK_DEPTH, CHUNK_HEIGHT)
}
