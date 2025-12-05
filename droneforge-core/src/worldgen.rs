use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use fastrand::Rng;

use crate::block::{AIR, BEDROCK, BlockId, DIRT, IRON, STONE};
use crate::chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk};
use crate::coordinates::{ChunkPosition, LocalBlockCoord, WorldCoord};

const HORIZONTAL_LIMIT: i32 = 1024;
const VERTICAL_LIMIT: i32 = 65;

#[derive(Debug, Clone)]
pub struct DeterministicMap {
    seed: u64,
}

impl DeterministicMap {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub fn block_at(&self, coord: WorldCoord) -> BlockId {
        if !self.within_bounds(coord) {
            return AIR;
        }

        if coord.z == VERTICAL_LIMIT {
            return AIR;
        }

        if coord.z == -VERTICAL_LIMIT {
            return BEDROCK;
        }

        if coord.z > 0 {
            if coord.x > 0 && coord.y == coord.z {
                return STONE;
            }
            return AIR;
        }

        if coord.x < 0 {
            if coord.z == 0 && (-5..=0).contains(&coord.x) {
                return DIRT;
            }

            if (-64..=-5).contains(&coord.z) {
                return STONE;
            }

            return AIR;
        }

        if coord.x > 0 && (-64..=0).contains(&coord.z) {
            let mut rng = self.rng_for_coord(coord);
            if rng.u32(0..100) < 5 {
                return IRON;
            }
            return STONE;
        }

        AIR
    }

    pub fn chunk_for_position(&self, position: ChunkPosition) -> Chunk {
        let mut chunk = Chunk::new(position, AIR);
        let base_x = position.x * CHUNK_WIDTH as i32;
        let base_y = position.y * CHUNK_DEPTH as i32;
        let base_z = position.z * CHUNK_HEIGHT as i32;

        for z in 0..CHUNK_HEIGHT {
            for y in 0..CHUNK_DEPTH {
                for x in 0..CHUNK_WIDTH {
                    let world_coord =
                        WorldCoord::new(base_x + x as i32, base_y + y as i32, base_z + z as i32);

                    let block = self.block_at(world_coord);
                    let local = LocalBlockCoord::new(x, y, z);
                    chunk
                        .set_block(local, block)
                        .expect("Local coordinate must be in chunk bounds");
                }
            }
        }

        chunk
    }

    fn rng_for_coord(&self, coord: WorldCoord) -> Rng {
        let mut hasher = DefaultHasher::new();
        self.seed.hash(&mut hasher);
        coord.hash(&mut hasher);
        let seed = hasher.finish();
        Rng::with_seed(seed)
    }

    fn within_bounds(&self, coord: WorldCoord) -> bool {
        (-HORIZONTAL_LIMIT..=HORIZONTAL_LIMIT).contains(&coord.x)
            && (-HORIZONTAL_LIMIT..=HORIZONTAL_LIMIT).contains(&coord.y)
            && (-VERTICAL_LIMIT..=VERTICAL_LIMIT).contains(&coord.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> DeterministicMap {
        DeterministicMap::new(42)
    }

    #[test]
    fn enforces_bounds() {
        let generator = map();
        assert_eq!(generator.block_at(WorldCoord::new(2000, 0, 0)), AIR);
        assert_eq!(generator.block_at(WorldCoord::new(0, 2000, 0)), AIR);
        assert_eq!(generator.block_at(WorldCoord::new(0, 0, 200)), AIR);
    }

    #[test]
    fn produces_bedrock_and_air_caps() {
        let generator = map();
        assert_eq!(generator.block_at(WorldCoord::new(0, 0, 65)), AIR);
        assert_eq!(generator.block_at(WorldCoord::new(0, 0, -65)), BEDROCK);
    }

    #[test]
    fn places_left_side_layers() {
        let generator = map();
        assert_eq!(generator.block_at(WorldCoord::new(-3, 0, 0)), DIRT);
        assert_eq!(generator.block_at(WorldCoord::new(-3, 0, -5)), STONE);
        assert_eq!(generator.block_at(WorldCoord::new(-3, 0, -64)), STONE);
    }

    #[test]
    fn places_right_side_ores_deterministically() {
        let generator = map();
        let mut iron_coord = None;

        for x in 1..100 {
            let candidate = WorldCoord::new(x, 0, -10);
            if generator.block_at(candidate) == IRON {
                iron_coord = Some(candidate);
                break;
            }
        }

        let iron_coord = iron_coord.expect("expected at least one iron vein along the right side");
        assert_eq!(generator.block_at(iron_coord), IRON);

        let different_seed = DeterministicMap::new(7);
        let diverges = (1..20).any(|x| {
            let coord = WorldCoord::new(x, 0, -10);
            generator.block_at(coord) != different_seed.block_at(coord)
        });

        assert!(diverges);
    }

    #[test]
    fn stone_above_ground_follows_diagonal_rule() {
        let generator = map();
        assert_eq!(generator.block_at(WorldCoord::new(1, 3, 3)), STONE);
        assert_eq!(generator.block_at(WorldCoord::new(1, 2, 3)), AIR);
        assert_eq!(generator.block_at(WorldCoord::new(-1, 3, 3)), AIR);
    }

    #[test]
    fn chunk_generation_matches_block_at() {
        let generator = map();
        let position = ChunkPosition::new(0, 0, 0);
        let chunk = generator.chunk_for_position(position);

        let local = LocalBlockCoord::new(13, 0, 0);
        assert_eq!(
            chunk.get_block(local).unwrap(),
            generator.block_at(WorldCoord::new(13, 0, 0))
        );

        let local_ore = LocalBlockCoord::new(10, 0, 0);
        assert_eq!(
            chunk.get_block(local_ore).unwrap(),
            generator.block_at(WorldCoord::new(10, 0, 0))
        );
    }
}
