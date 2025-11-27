use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChunkPosition {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalBlockCoord {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

impl LocalBlockCoord {
    pub fn new(x: usize, y: usize, z: usize) -> Self {
        Self { x, y, z }
    }
}
