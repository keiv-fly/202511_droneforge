use crate::block::{AIR, BlockId};
use crate::chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk, ChunkBlocks, ChunkError};
use crate::coordinates::{ChunkPosition, LocalBlockCoord, WorldCoord};
use crate::linecast::first_solid_supercover;
use crate::worldgen::{DeterministicMap, HORIZONTAL_LIMIT, VERTICAL_LIMIT};
use std::collections::HashMap;
use std::ops::RangeInclusive;

#[derive(Debug, Clone)]
pub struct CachedChunk {
    pub position: ChunkPosition,
    pub palette: Vec<BlockId>,
    blocks: Vec<u64>,
    bits_per_index: u8,
    changed: bool,
}

impl CachedChunk {
    pub fn from_chunk(chunk: &Chunk) -> Self {
        Self::from_chunk_with_changed(chunk, false)
    }

    pub fn from_chunk_with_changed(chunk: &Chunk, changed: bool) -> Self {
        let (palette, palette_indices) = build_palette(chunk.blocks());
        let bits_per_index = bits_required(palette.len());
        let blocks = pack_blocks(chunk.blocks(), &palette_indices, bits_per_index);

        Self {
            position: chunk.position,
            palette,
            blocks,
            bits_per_index,
            changed,
        }
    }

    pub fn get_block(&self, coord: LocalBlockCoord) -> Result<BlockId, ChunkError> {
        if self.palette.is_empty() {
            return Err(ChunkError::InvalidBlockCount(0));
        }

        let index = Chunk::block_index(coord)?;
        if self.bits_per_index == 0 {
            return Ok(self.palette[0]);
        }

        let palette_index = unpack_index(&self.blocks, index, self.bits_per_index);
        self.palette
            .get(palette_index as usize)
            .copied()
            .ok_or(ChunkError::InvalidBlockCount(palette_index as usize))
    }

    pub fn changed(&self) -> bool {
        self.changed
    }
}

#[derive(Debug, Clone)]
pub struct ChunkCache {
    chunks: HashMap<ChunkPosition, CachedChunk>,
    reusable_chunk: Chunk,
    last_save_ms: Option<f64>,
}

impl ChunkCache {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            chunks: HashMap::with_capacity(capacity),
            reusable_chunk: Chunk::new(ChunkPosition::new(0, 0, 0), AIR),
            last_save_ms: None,
        }
    }

    pub fn from_generator_with_limits(generator: &DeterministicMap) -> Self {
        let total_chunks = Self::chunk_count_for_limits(HORIZONTAL_LIMIT, VERTICAL_LIMIT);
        let mut cache = Self::with_capacity(total_chunks);
        cache.populate_within_limits(generator, HORIZONTAL_LIMIT, VERTICAL_LIMIT);
        cache
    }

    pub fn from_generator_with_limits_with_progress(
        generator: &DeterministicMap,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Self {
        let total_chunks = Self::chunk_count_for_limits(HORIZONTAL_LIMIT, VERTICAL_LIMIT);
        let mut cache = Self::with_capacity(total_chunks);
        cache.populate_within_limits_with_progress(
            generator,
            HORIZONTAL_LIMIT,
            VERTICAL_LIMIT,
            Some(progress),
        );
        cache
    }

    pub fn populate_within_limits(
        &mut self,
        generator: &DeterministicMap,
        horizontal_limit: i32,
        vertical_limit: i32,
    ) {
        self.populate_within_limits_with_progress(
            generator,
            horizontal_limit,
            vertical_limit,
            None,
        );
    }

    pub fn populate_within_limits_with_progress(
        &mut self,
        generator: &DeterministicMap,
        horizontal_limit: i32,
        vertical_limit: i32,
        mut progress: Option<&mut dyn FnMut(usize, usize)>,
    ) {
        let (chunk_xs, chunk_ys, chunk_zs) =
            Self::chunk_ranges_for_limits(horizontal_limit, vertical_limit);
        let total_chunks = chunk_xs.len() * chunk_ys.len() * chunk_zs.len();
        self.chunks
            .reserve(total_chunks.saturating_sub(self.chunks.len()));
        let mut loaded = 0usize;
        let notify_every = 1000usize;
        let mut last_notified = 0usize;

        for &chunk_x in &chunk_xs {
            for &chunk_y in &chunk_ys {
                for &chunk_z in &chunk_zs {
                    let position = ChunkPosition::new(chunk_x, chunk_y, chunk_z);
                    self.populate_chunk_at(generator, position);
                    loaded += 1;

                    if let Some(callback) = progress.as_mut() {
                        if loaded % notify_every == 0 || loaded == total_chunks {
                            callback(loaded, total_chunks);
                            last_notified = loaded;
                        }
                    }
                }
            }
        }

        if let Some(callback) = progress.as_mut() {
            if loaded > 0 && loaded != last_notified {
                callback(loaded, total_chunks);
            }
        }
    }

    pub fn populate_chunk_at(&mut self, generator: &DeterministicMap, position: ChunkPosition) {
        generator.populate_chunk(&mut self.reusable_chunk, position);
        let cached = CachedChunk::from_chunk(&self.reusable_chunk);
        self.chunks.insert(position, cached);
    }

    pub fn chunk_ranges_for_limits(
        horizontal_limit: i32,
        vertical_limit: i32,
    ) -> (Vec<i32>, Vec<i32>, Vec<i32>) {
        let (x_range, y_range, z_range) = chunk_range_bounds(horizontal_limit, vertical_limit);
        (x_range.collect(), y_range.collect(), z_range.collect())
    }

    pub fn chunk_count_for_limits(horizontal_limit: i32, vertical_limit: i32) -> usize {
        let (x_range, y_range, z_range) = chunk_range_bounds(horizontal_limit, vertical_limit);
        x_range.count() * y_range.count() * z_range.count()
    }

    pub fn chunk(&self, position: &ChunkPosition) -> Option<&CachedChunk> {
        self.chunks.get(position)
    }

    pub fn has_chunk(&self, position: &ChunkPosition) -> bool {
        self.chunks.contains_key(position)
    }

    pub fn has_level(&self, chunk_z: i32) -> bool {
        self.chunks.keys().any(|pos| pos.z == chunk_z)
    }

    pub fn block_at_world(&self, coord: WorldCoord) -> Option<BlockId> {
        let (chunk_pos, local) = chunk_and_local_for_world_coord(coord);
        self.chunk(&chunk_pos)
            .and_then(|chunk| chunk.get_block(local).ok())
    }

    pub fn set_block(&mut self, coord: WorldCoord, block: BlockId) -> Result<(), ChunkError> {
        let (chunk_pos, local) = chunk_and_local_for_world_coord(coord);
        let Some(existing) = self.chunks.get(&chunk_pos) else {
            return Err(ChunkError::OutOfBounds);
        };

        let mut chunk = chunk_from_cached(existing)?;
        chunk.set_block(local, block)?;

        let updated = CachedChunk::from_chunk_with_changed(&chunk, true);
        self.chunks.insert(chunk_pos, updated);
        Ok(())
    }

    pub fn last_save_ms(&self) -> Option<f64> {
        self.last_save_ms
    }

    pub fn record_save_time_ms(&mut self, ms: f64) {
        if ms.is_finite() && ms >= 0.0 {
            self.last_save_ms = Some(ms);
        }
    }

    /// Returns the first solid block encountered along the line from `start` to `end`,
    /// using a supercover Bresenham traversal across the XY plane. The start tile is
    /// ignored, allowing a drone to move out of its current tile even if occupied.
    pub fn first_solid_on_line(&self, start: WorldCoord, end: WorldCoord) -> Option<WorldCoord> {
        first_solid_supercover(|coord| self.block_at_world(coord), start, end)
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }
}

impl Default for ChunkCache {
    fn default() -> Self {
        Self::new()
    }
}

fn build_palette(blocks: &[BlockId]) -> (Vec<BlockId>, HashMap<BlockId, usize>) {
    let mut palette = Vec::new();
    let mut indices = HashMap::new();

    for &block in blocks {
        if !indices.contains_key(&block) {
            let idx = palette.len();
            palette.push(block);
            indices.insert(block, idx);
        }
    }

    (palette, indices)
}

fn bits_required(palette_len: usize) -> u8 {
    if palette_len <= 1 {
        0
    } else {
        (usize::BITS - (palette_len.saturating_sub(1)).leading_zeros()) as u8
    }
}

fn pack_blocks(
    blocks: &[BlockId],
    palette_indices: &HashMap<BlockId, usize>,
    bits_per_index: u8,
) -> Vec<u64> {
    if bits_per_index == 0 {
        return Vec::new();
    }

    let total_bits = blocks.len() * bits_per_index as usize;
    let u64_len = (total_bits + 63) / 64;
    let mut packed = vec![0u64; u64_len];
    let mask: u64 = (1u64 << bits_per_index) - 1;

    for (i, block) in blocks.iter().enumerate() {
        let palette_index = *palette_indices
            .get(block)
            .expect("palette must contain block");
        let offset = i * bits_per_index as usize;
        let word_index = offset / 64;
        let bit_in_word = offset % 64;
        let value = (palette_index as u64) & mask;

        packed[word_index] |= value << bit_in_word;
        let bits_written = bit_in_word + bits_per_index as usize;
        if bits_written > 64 {
            let spill_bits = bits_written - 64;
            packed[word_index + 1] |= value >> (bits_per_index as usize - spill_bits);
        }
    }

    packed
}

fn unpack_index(packed: &[u64], block_index: usize, bits_per_index: u8) -> u64 {
    let offset = block_index * bits_per_index as usize;
    let word_index = offset / 64;
    let bit_in_word = offset % 64;
    let mask = (1u64 << bits_per_index) - 1;

    let mut value = (packed[word_index] >> bit_in_word) & mask;
    let bits_read = bit_in_word + bits_per_index as usize;
    if bits_read > 64 {
        let spill_bits = bits_read - 64;
        let spill_mask = (1u64 << spill_bits) - 1;
        let spill = packed[word_index + 1] & spill_mask;
        value |= spill << (bits_per_index as usize - spill_bits);
    }

    value
}

fn chunk_and_local_for_world_coord(coord: WorldCoord) -> (ChunkPosition, LocalBlockCoord) {
    let chunk_x = div_floor(coord.x, CHUNK_WIDTH as i32);
    let chunk_y = div_floor(coord.y, CHUNK_DEPTH as i32);
    let chunk_z = div_floor(coord.z, CHUNK_HEIGHT as i32);

    let local_x = (coord.x - chunk_x * CHUNK_WIDTH as i32) as usize;
    let local_y = (coord.y - chunk_y * CHUNK_DEPTH as i32) as usize;
    let local_z = (coord.z - chunk_z * CHUNK_HEIGHT as i32) as usize;

    (
        ChunkPosition::new(chunk_x, chunk_y, chunk_z),
        LocalBlockCoord::new(local_x, local_y, local_z),
    )
}

fn chunkblocks_from_cached(cached: &CachedChunk) -> Vec<BlockId> {
    let mut blocks = Vec::with_capacity(CHUNK_WIDTH * CHUNK_DEPTH * CHUNK_HEIGHT);
    for z in 0..CHUNK_HEIGHT {
        for y in 0..CHUNK_DEPTH {
            for x in 0..CHUNK_WIDTH {
                let coord = LocalBlockCoord::new(x, y, z);
                let block = cached.get_block(coord).unwrap_or(AIR);
                blocks.push(block);
            }
        }
    }
    blocks
}

fn chunk_from_cached(cached: &CachedChunk) -> Result<Chunk, ChunkError> {
    let mut chunk = Chunk::new(cached.position, AIR);
    let blocks = chunkblocks_from_cached(cached);
    let blocks_struct = ChunkBlocks {
        position: cached.position,
        blocks,
        changed: cached.changed,
    };
    chunk.apply_block_save(&blocks_struct)?;
    Ok(chunk)
}

fn chunk_range_bounds(
    horizontal_limit: i32,
    vertical_limit: i32,
) -> (
    RangeInclusive<i32>,
    RangeInclusive<i32>,
    RangeInclusive<i32>,
) {
    let min_chunk_x = div_floor(-horizontal_limit, CHUNK_WIDTH as i32);
    let max_chunk_x = div_floor(horizontal_limit, CHUNK_WIDTH as i32);
    let min_chunk_y = div_floor(-horizontal_limit, CHUNK_DEPTH as i32);
    let max_chunk_y = div_floor(horizontal_limit, CHUNK_DEPTH as i32);
    let min_chunk_z = div_floor(-vertical_limit, CHUNK_HEIGHT as i32);
    let max_chunk_z = div_floor(vertical_limit, CHUNK_HEIGHT as i32);

    (
        min_chunk_x..=max_chunk_x,
        min_chunk_y..=max_chunk_y,
        min_chunk_z..=max_chunk_z,
    )
}

fn div_floor(a: i32, b: i32) -> i32 {
    let (d, r) = (a / b, a % b);
    if r != 0 && ((r < 0) != (b < 0)) {
        d - 1
    } else {
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{AIR, BEDROCK, DIRT, IRON, STONE};

    fn coord_for_index(index: usize) -> LocalBlockCoord {
        let z = index / (CHUNK_WIDTH * CHUNK_DEPTH);
        let remainder = index % (CHUNK_WIDTH * CHUNK_DEPTH);
        let y = remainder / CHUNK_WIDTH;
        let x = remainder % CHUNK_WIDTH;
        LocalBlockCoord::new(x, y, z)
    }

    fn cache_with_solid_blocks(coords: &[WorldCoord]) -> ChunkCache {
        let mut chunks: HashMap<ChunkPosition, Chunk> = HashMap::new();

        for &coord in coords {
            let (chunk_pos, local) = chunk_and_local_for_world_coord(coord);
            let chunk = chunks
                .entry(chunk_pos)
                .or_insert_with(|| Chunk::new(chunk_pos, AIR));
            chunk.set_block(local, STONE).unwrap();
        }

        let mut cache = ChunkCache::new();
        for (pos, chunk) in chunks {
            let cached = CachedChunk::from_chunk(&chunk);
            cache.chunks.insert(pos, cached);
        }

        cache
    }

    #[test]
    fn single_palette_uses_no_storage() {
        let position = ChunkPosition::new(0, 0, 0);
        let chunk = Chunk::new(position, AIR);
        let cached = CachedChunk::from_chunk(&chunk);

        assert_eq!(cached.palette, vec![AIR]);
        assert!(cached.blocks.is_empty());
        assert_eq!(
            cached
                .get_block(LocalBlockCoord::new(0, 0, 0))
                .expect("block should exist"),
            AIR
        );
    }

    #[test]
    fn retrieves_blocks_across_word_boundaries() {
        let position = ChunkPosition::new(1, 0, 0);
        let mut chunk = Chunk::new(position, AIR);

        let palette_values: [BlockId; 5] = [AIR, DIRT, STONE, IRON, BEDROCK];
        for (i, &block) in palette_values.iter().enumerate() {
            let coord = coord_for_index(i);
            chunk.set_block(coord, block).unwrap();
        }

        let boundary_index = 22;
        let boundary_coord = coord_for_index(boundary_index);
        chunk.set_block(boundary_coord, BEDROCK).unwrap();

        let cached = CachedChunk::from_chunk(&chunk);
        assert!(cached.palette.len() >= palette_values.len());

        for (i, &block) in palette_values.iter().enumerate() {
            let coord = coord_for_index(i);
            assert_eq!(cached.get_block(coord).unwrap(), block);
        }
        assert_eq!(cached.get_block(boundary_coord).unwrap(), BEDROCK);
    }

    #[test]
    fn cache_serves_world_coordinates() {
        let generator = DeterministicMap::new(7);
        let mut cache = ChunkCache::new();
        cache.populate_within_limits(&generator, CHUNK_WIDTH as i32, CHUNK_HEIGHT as i32);

        let coord = WorldCoord::new(0, 0, 0);
        let expected = generator.block_at(coord);
        assert_eq!(cache.block_at_world(coord), Some(expected));
    }

    #[test]
    fn set_block_updates_changed_chunks_and_time() {
        let generator = DeterministicMap::new(3);
        let mut cache = ChunkCache::new();
        cache.populate_within_limits(&generator, CHUNK_WIDTH as i32, CHUNK_HEIGHT as i32);

        let coord = WorldCoord::new(0, 0, 0);
        let original = cache.block_at_world(coord).expect("block should exist");

        cache.set_block(coord, AIR).unwrap();
        assert_eq!(cache.block_at_world(coord), Some(AIR));
        cache.record_save_time_ms(1.5);
        assert_eq!(cache.last_save_ms(), Some(1.5));
        assert!(
            cache
                .chunk(&ChunkPosition::new(0, 0, 0))
                .map(|c| c.changed())
                .unwrap_or(false)
        );

        cache.set_block(coord, original).unwrap();
        assert_eq!(cache.block_at_world(coord), Some(original));
        cache.record_save_time_ms(0.0);
        assert_eq!(cache.last_save_ms(), Some(0.0));
        assert!(
            cache
                .chunk(&ChunkPosition::new(0, 0, 0))
                .map(|c| c.changed())
                .unwrap_or(false)
        );
    }

    #[test]
    fn palette_builds_unique_ids_in_order() {
        let blocks = vec![AIR, DIRT, AIR, STONE, DIRT];
        let (palette, indices) = build_palette(&blocks);

        assert_eq!(palette, vec![AIR, DIRT, STONE]);
        assert_eq!(indices[&AIR], 0);
        assert_eq!(indices[&DIRT], 1);
        assert_eq!(indices[&STONE], 2);
    }

    #[test]
    fn bits_required_matches_palette_size() {
        assert_eq!(bits_required(0), 0);
        assert_eq!(bits_required(1), 0);
        assert_eq!(bits_required(2), 1);
        assert_eq!(bits_required(3), 2);
        assert_eq!(bits_required(4), 2);
    }

    #[test]
    fn pack_and_unpack_restore_palette_indices() {
        let blocks: Vec<BlockId> = vec![AIR, DIRT, STONE, IRON, STONE, DIRT, AIR, IRON];
        let (palette, indices) = build_palette(&blocks);
        let bits_per_index = bits_required(palette.len());
        let packed = pack_blocks(&blocks, &indices, bits_per_index);

        for (i, original) in blocks.iter().enumerate() {
            let palette_index = unpack_index(&packed, i, bits_per_index);
            let restored = palette[palette_index as usize];
            assert_eq!(restored, *original);
        }
    }

    #[test]
    fn chunk_and_local_calculations_cover_negative_world_coords() {
        let world = WorldCoord::new(-1, -1, -1);
        let (chunk_pos, local) = chunk_and_local_for_world_coord(world);

        assert_eq!(chunk_pos, ChunkPosition::new(-1, -1, -1));
        assert_eq!(
            local,
            LocalBlockCoord::new(CHUNK_WIDTH - 1, CHUNK_DEPTH - 1, CHUNK_HEIGHT - 1)
        );
    }

    #[test]
    fn chunk_count_matches_ranges() {
        let horizontal_limit = CHUNK_WIDTH as i32; // produces three chunk positions (-1, 0, 1)
        let vertical_limit = CHUNK_HEIGHT as i32; // produces three chunk positions (-1, 0, 1)

        let (xs, ys, zs) = ChunkCache::chunk_ranges_for_limits(horizontal_limit, vertical_limit);
        let expected = xs.len() * ys.len() * zs.len();

        assert_eq!(
            ChunkCache::chunk_count_for_limits(horizontal_limit, vertical_limit),
            expected
        );
    }

    #[test]
    fn detects_collision_along_straight_line() {
        let cache = cache_with_solid_blocks(&[WorldCoord::new(2, 0, 0)]);

        let start = WorldCoord::new(0, 0, 0);
        let end = WorldCoord::new(4, 0, 0);

        assert_eq!(
            cache.first_solid_on_line(start, end),
            Some(WorldCoord::new(2, 0, 0))
        );
    }

    #[test]
    fn detects_diagonal_neighbour_collision_when_error_zero() {
        let cache = cache_with_solid_blocks(&[WorldCoord::new(1, 0, 0)]);

        let start = WorldCoord::new(0, 0, 0);
        let end = WorldCoord::new(2, 2, 0);

        assert_eq!(
            cache.first_solid_on_line(start, end),
            Some(WorldCoord::new(1, 0, 0))
        );
    }

    #[test]
    fn clear_path_returns_none() {
        let cache = cache_with_solid_blocks(&[]);

        let start = WorldCoord::new(-1, -1, 0);
        let end = WorldCoord::new(3, 2, 0);

        assert_eq!(cache.first_solid_on_line(start, end), None);
    }
}
