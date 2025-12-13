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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl WorldCoord {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn constructors_assign_fields() {
        let chunk_pos = ChunkPosition::new(1, -2, 3);
        assert_eq!((chunk_pos.x, chunk_pos.y, chunk_pos.z), (1, -2, 3));

        let local = LocalBlockCoord::new(4, 5, 6);
        assert_eq!((local.x, local.y, local.z), (4, 5, 6));

        let world = WorldCoord::new(-7, 8, -9);
        assert_eq!((world.x, world.y, world.z), (-7, 8, -9));
    }

    #[test]
    fn coordinates_work_as_hash_keys() {
        let mut chunks = HashSet::new();
        let a = ChunkPosition::new(0, 0, 0);
        let b = ChunkPosition::new(1, 0, 0);
        chunks.insert(a);
        assert!(chunks.contains(&a));
        assert!(!chunks.contains(&b));

        let mut world_coords = HashSet::new();
        let c = WorldCoord::new(10, -5, 2);
        let d = WorldCoord::new(10, -5, 3);
        world_coords.insert(c);
        assert!(world_coords.contains(&c));
        assert!(!world_coords.contains(&d));
    }
}
