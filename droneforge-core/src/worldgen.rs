use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use fastrand::Rng;

use crate::block::{AIR, BEDROCK, BlockId, DIRT, IRON, STONE};
use crate::chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk};
use crate::coordinates::{ChunkPosition, LocalBlockCoord, WorldCoord};

pub const HORIZONTAL_LIMIT: i32 = 1024;
pub const VERTICAL_LIMIT: i32 = 65;

#[derive(Debug, Clone)]
pub struct DeterministicMap {
    seed: u64,
}

impl DeterministicMap {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub fn block_at(&self, coord: WorldCoord) -> BlockId {
        let shifted = WorldCoord::new(coord.x, coord.y, coord.z.saturating_add(1));

        if !self.within_bounds(shifted) {
            return AIR;
        }

        if shifted.z == VERTICAL_LIMIT {
            return AIR;
        }

        if shifted.z == -VERTICAL_LIMIT {
            return BEDROCK;
        }

        if shifted.x <= 0 {
            if (-4..=0).contains(&shifted.z) {
                return DIRT;
            }

            if (-64..=-5).contains(&shifted.z) {
                let mut rng = self.rng_for_coord(shifted);
                if rng.u32(0..100) < 5 {
                    return IRON;
                }
                return STONE;
            }

            return AIR;
        }

        if (shifted.x > 0 && (-64..=0).contains(&shifted.z))
            || (shifted.x - shifted.z + 1 > 0 && shifted.z > 0)
        {
            let mut rng = self.rng_for_coord(shifted);
            if rng.u32(0..100) < 5 {
                return IRON;
            }
            return STONE;
        }

        AIR
    }

    pub fn chunk_for_position(&self, position: ChunkPosition) -> Chunk {
        let mut chunk = Chunk::new(position, AIR);
        self.populate_chunk(&mut chunk, position);
        chunk
    }

    pub fn populate_chunk(&self, chunk: &mut Chunk, position: ChunkPosition) {
        chunk.position = position;
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
        assert_eq!(generator.block_at(WorldCoord::new(0, 0, 64)), AIR);
        assert_eq!(generator.block_at(WorldCoord::new(0, 0, -66)), BEDROCK);
    }

    #[test]
    fn places_left_side_layers() {
        let generator = map();
        assert_eq!(generator.block_at(WorldCoord::new(-3, 0, -1)), DIRT);
        assert_eq!(generator.block_at(WorldCoord::new(-3, 0, -6)), STONE);
        assert_eq!(generator.block_at(WorldCoord::new(-3, 0, -65)), STONE);
    }

    #[test]
    fn places_right_side_ores_deterministically() {
        let generator = map();
        let mut iron_coord = None;

        for x in 1..100 {
            let candidate = WorldCoord::new(x, 0, -11);
            if generator.block_at(candidate) == IRON {
                iron_coord = Some(candidate);
                break;
            }
        }

        let iron_coord = iron_coord.expect("expected at least one iron vein along the right side");
        assert_eq!(generator.block_at(iron_coord), IRON);

        let different_seed = DeterministicMap::new(7);
        let diverges = (1..20).any(|x| {
            let coord = WorldCoord::new(x, 0, -11);
            generator.block_at(coord) != different_seed.block_at(coord)
        });

        assert!(diverges);
    }

    #[test]
    fn stone_above_ground_follows_diagonal_rule() {
        let generator = map();
        // For z > 0 the area where x - z + 1 > 0 should contain stone, otherwise air.
        assert_eq!(generator.block_at(WorldCoord::new(4, 0, 1)), STONE);
        assert_eq!(generator.block_at(WorldCoord::new(1, 0, 2)), AIR);
        assert_eq!(generator.block_at(WorldCoord::new(-1, 0, 2)), AIR);
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
