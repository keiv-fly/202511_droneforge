use droneforge_core::{BEDROCK, BlockId, DIRT, IRON, STONE};

pub const BLOCK_PIXEL_SIZE: u32 = 16;
pub const TILESET_COLUMNS: u32 = 8;
pub const WALL_MASK_VARIANTS: u32 = 16;

pub const MASK_NORTH: u8 = 1;
pub const MASK_EAST: u8 = 2;
pub const MASK_SOUTH: u8 = 4;
pub const MASK_WEST: u8 = 8;

pub const SOLID_BLOCKS: [BlockId; 4] = [DIRT, STONE, IRON, BEDROCK];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TilePosition {
    pub tile_x: u32,
    pub tile_y: u32,
}

pub fn solid_block_index(block: BlockId) -> Option<u32> {
    SOLID_BLOCKS
        .iter()
        .position(|&candidate| candidate == block)
        .map(|idx| idx as u32)
}

pub const fn solid_block_count() -> u32 {
    SOLID_BLOCKS.len() as u32
}

pub const fn floor_tile_count() -> u32 {
    solid_block_count()
}

pub const fn total_tile_count() -> u32 {
    floor_tile_count() + solid_block_count() * WALL_MASK_VARIANTS
}

pub const fn tile_grid_size() -> (u32, u32) {
    let columns = TILESET_COLUMNS;
    let rows = (total_tile_count() + columns - 1) / columns;
    (columns, rows)
}

pub const fn atlas_pixel_size() -> (u32, u32) {
    let (columns, rows) = tile_grid_size();
    (columns * BLOCK_PIXEL_SIZE, rows * BLOCK_PIXEL_SIZE)
}

pub fn floor_tile_index(block: BlockId) -> Option<u32> {
    solid_block_index(block)
}

pub fn wall_tile_index(block: BlockId, mask: u8) -> Option<u32> {
    if mask >= WALL_MASK_VARIANTS as u8 {
        return None;
    }

    solid_block_index(block)
        .map(|block_idx| floor_tile_count() + block_idx * WALL_MASK_VARIANTS + mask as u32)
}

pub fn floor_tile_position(block: BlockId) -> Option<TilePosition> {
    floor_tile_index(block).map(tile_position_for_index)
}

pub fn wall_tile_position(block: BlockId, mask: u8) -> Option<TilePosition> {
    wall_tile_index(block, mask).map(tile_position_for_index)
}

pub fn tile_position_for_index(tile_index: u32) -> TilePosition {
    let tile_x = tile_index % TILESET_COLUMNS;
    let tile_y = tile_index / TILESET_COLUMNS;
    TilePosition { tile_x, tile_y }
}

pub fn tile_pixel_position(tile_index: u32) -> (u32, u32) {
    let pos = tile_position_for_index(tile_index);
    (pos.tile_x * BLOCK_PIXEL_SIZE, pos.tile_y * BLOCK_PIXEL_SIZE)
}
