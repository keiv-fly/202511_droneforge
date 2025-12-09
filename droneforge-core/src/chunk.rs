use crate::block::BlockId;
use crate::coordinates::{ChunkPosition, LocalBlockCoord};
use serde::{Deserialize, Serialize};

pub const CHUNK_WIDTH: usize = 32;
pub const CHUNK_DEPTH: usize = 32;
pub const CHUNK_HEIGHT: usize = 4;
const CHUNK_BLOCKS: usize = CHUNK_WIDTH * CHUNK_DEPTH * CHUNK_HEIGHT;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkBlocks {
    pub position: ChunkPosition,
    pub blocks: Vec<BlockId>,
}

impl ChunkBlocks {
    pub fn new(position: ChunkPosition, blocks: Vec<BlockId>) -> Result<Self, ChunkError> {
        if blocks.len() != CHUNK_BLOCKS {
            return Err(ChunkError::InvalidBlockCount(blocks.len()));
        }
        Ok(Self { position, blocks })
    }
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub position: ChunkPosition,
    blocks: Vec<BlockId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkError {
    InvalidBlockCount(usize),
    OutOfBounds,
}

impl Chunk {
    pub fn new(position: ChunkPosition, default_block: BlockId) -> Self {
        Self {
            position,
            blocks: vec![default_block; CHUNK_BLOCKS],
        }
    }

    pub fn block_index(coord: LocalBlockCoord) -> Result<usize, ChunkError> {
        if coord.x >= CHUNK_WIDTH || coord.y >= CHUNK_DEPTH || coord.z >= CHUNK_HEIGHT {
            return Err(ChunkError::OutOfBounds);
        }
        Ok(coord.x + coord.y * CHUNK_WIDTH + coord.z * CHUNK_WIDTH * CHUNK_DEPTH)
    }

    pub fn get_block(&self, coord: LocalBlockCoord) -> Result<BlockId, ChunkError> {
        let index = Self::block_index(coord)?;
        Ok(self.blocks[index])
    }

    pub fn set_block(&mut self, coord: LocalBlockCoord, block: BlockId) -> Result<(), ChunkError> {
        let index = Self::block_index(coord)?;
        self.blocks[index] = block;
        Ok(())
    }

    pub fn to_block_save(&self) -> ChunkBlocks {
        ChunkBlocks {
            position: self.position,
            blocks: self.blocks.clone(),
        }
    }

    pub fn blocks(&self) -> &[BlockId] {
        &self.blocks
    }

    pub fn apply_block_save(&mut self, data: &ChunkBlocks) -> Result<(), ChunkError> {
        if data.blocks.len() != CHUNK_BLOCKS {
            return Err(ChunkError::InvalidBlockCount(data.blocks.len()));
        }
        self.blocks.clone_from(&data.blocks);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_index_respects_coordinate_order() {
        let coord = LocalBlockCoord::new(1, 2, 3);
        let index = Chunk::block_index(coord).unwrap();
        let expected = 1 + 2 * CHUNK_WIDTH + 3 * CHUNK_WIDTH * CHUNK_DEPTH;
        assert_eq!(index, expected);
    }

    #[test]
    fn set_and_get_block() {
        let position = ChunkPosition::new(0, 0, 0);
        let mut chunk = Chunk::new(position, 0);
        let coord = LocalBlockCoord::new(0, 0, 0);
        chunk.set_block(coord, 5).unwrap();
        assert_eq!(chunk.get_block(coord).unwrap(), 5);
    }

    #[test]
    fn block_save_round_trip() {
        let position = ChunkPosition::new(1, 2, 3);
        let mut chunk = Chunk::new(position, 0);
        let coord = LocalBlockCoord::new(3, 4, 1);
        chunk.set_block(coord, 42).unwrap();

        let save = chunk.to_block_save();
        let mut restored = Chunk::new(position, 0);
        restored.apply_block_save(&save).unwrap();

        assert_eq!(restored.get_block(coord).unwrap(), 42);
    }

    #[test]
    fn rejects_invalid_block_count() {
        let position = ChunkPosition::new(0, 0, 0);
        let mut chunk = Chunk::new(position, 0);
        let mut data = chunk.to_block_save();
        data.blocks.pop();
        assert!(matches!(
            chunk.apply_block_save(&data),
            Err(ChunkError::InvalidBlockCount(_))
        ));
    }
}
