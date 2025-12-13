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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_error_wraps_message() {
        let error = StorageError::new("failed to load");
        assert_eq!(error.0, "failed to load");
    }

    #[test]
    fn expected_block_count_multiplies_dimensions() {
        assert_eq!(expected_block_count(2, 3, 4), 24);
        assert_eq!(expected_block_count(1, 1, 1), 1);
    }

    #[test]
    fn default_block_count_matches_chunk_constants() {
        let expected = CHUNK_WIDTH * CHUNK_DEPTH * CHUNK_HEIGHT;
        assert_eq!(default_block_count(), expected);
    }
}
