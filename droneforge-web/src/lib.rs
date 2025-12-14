use d_gen_tileset::layout::{self, MASK_EAST, MASK_NORTH, MASK_SOUTH, MASK_WEST};
use droneforge_core::chunk::CHUNK_HEIGHT;
use droneforge_core::worldgen::{DeterministicMap, HORIZONTAL_LIMIT, VERTICAL_LIMIT};
use droneforge_core::{
    AIR, BEDROCK, BlockId, ChunkCache, ChunkPosition, DronePose, World, WorldCoord,
};
#[cfg(target_arch = "wasm32")]
use macroquad::miniquad;
use macroquad::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::drone::{DroneDrawConfig, draw_drone, drone_world_center, is_visible_at_view};
const VIEW_MIN_X: i32 = -100;
const VIEW_MAX_X: i32 = 100;
const VIEW_MIN_Y: i32 = -60;
const VIEW_MAX_Y: i32 = 60;
const DEFAULT_VIEW_Z: i32 = 0;

const BLOCK_PIXEL_SIZE: u16 = layout::BLOCK_PIXEL_SIZE as u16;
const TILESET_PATH: &str = "assets/tileset.png";
const SPRITE_ATLAS_PATH: &str = "assets/sprites.png";
const DRONE_SPRITE_COLUMNS: u32 = 4;
const DRONE_SPRITE_ROWS: u32 = 4;
const DRONE_SPRITE_SIZE_PX: u32 = 256;
const DRONE_SPRITE_PADDING_PX: u32 = 2; // keep in sync with d-gen-tileset
const BASE_ZOOM_AT_POWER_ZERO: f32 = 1f32;
const MIN_ZOOM_POWER: i32 = -48;
const MAX_ZOOM_POWER: i32 = 15;
const ZOOM_FACTOR: f32 = 1.1;

mod drone;

const RENDER_CHUNK_SIZE: i32 = 32;
const PRELOAD_Z_RADIUS: i32 = 5;
const CHUNK_CACHE_CHUNKS_PER_FRAME: usize = 256;
const LOAD_METRIC_INTERVAL_SECS: f64 = 5.0;

static PENDING_Z_DELTA: AtomicI32 = AtomicI32::new(0);

static PENDING_MOVE_TOGGLE: AtomicBool = AtomicBool::new(false);
static MOVE_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);

static INITIAL_CHUNK_TOTAL: AtomicU32 = AtomicU32::new(0);
static INITIAL_CHUNK_LOADED: AtomicU32 = AtomicU32::new(0);

#[derive(Default)]
struct SelectedDroneUi {
    present: bool,
    name: String,
    health: i32,
    max_health: i32,
    status: String,
}

fn selected_drone_ui() -> &'static Mutex<SelectedDroneUi> {
    static SELECTED_DRONE_UI: OnceLock<Mutex<SelectedDroneUi>> = OnceLock::new();
    SELECTED_DRONE_UI.get_or_init(|| Mutex::new(SelectedDroneUi::default()))
}

#[unsafe(no_mangle)]
pub extern "C" fn z_level_up() {
    queue_z_delta(1);
}

#[unsafe(no_mangle)]
pub extern "C" fn z_level_down() {
    queue_z_delta(-1);
}

fn queue_z_delta(delta: i32) {
    PENDING_Z_DELTA.fetch_add(delta, Ordering::SeqCst);
}

fn take_pending_z_delta() -> i32 {
    PENDING_Z_DELTA.swap(0, Ordering::SeqCst)
}

#[unsafe(no_mangle)]
pub extern "C" fn chunk_cache_progress_fraction() -> f32 {
    let total = INITIAL_CHUNK_TOTAL.load(Ordering::SeqCst);
    if total == 0 {
        return 0.0;
    }

    let loaded = INITIAL_CHUNK_LOADED.load(Ordering::SeqCst).min(total);
    loaded as f32 / total as f32
}

#[unsafe(no_mangle)]
pub extern "C" fn chunk_cache_loaded_chunks() -> u32 {
    INITIAL_CHUNK_LOADED
        .load(Ordering::SeqCst)
        .min(INITIAL_CHUNK_TOTAL.load(Ordering::SeqCst))
}

#[unsafe(no_mangle)]
pub extern "C" fn chunk_cache_total_chunks() -> u32 {
    INITIAL_CHUNK_TOTAL.load(Ordering::SeqCst)
}

#[unsafe(no_mangle)]
pub extern "C" fn selected_drone_present() -> i32 {
    let ui = selected_drone_ui().lock().unwrap();
    if ui.present { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn selected_drone_name_ptr() -> *const u8 {
    let ui = selected_drone_ui().lock().unwrap();
    if ui.present {
        ui.name.as_ptr()
    } else {
        ptr::null()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn selected_drone_name_len() -> usize {
    let ui = selected_drone_ui().lock().unwrap();
    if ui.present { ui.name.len() } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn selected_drone_status_ptr() -> *const u8 {
    let ui = selected_drone_ui().lock().unwrap();
    if ui.present && !ui.status.is_empty() {
        ui.status.as_ptr()
    } else {
        ptr::null()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn selected_drone_status_len() -> usize {
    let ui = selected_drone_ui().lock().unwrap();
    if ui.present { ui.status.len() } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn selected_drone_health() -> i32 {
    let ui = selected_drone_ui().lock().unwrap();
    if ui.present { ui.health } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn selected_drone_health_max() -> i32 {
    let ui = selected_drone_ui().lock().unwrap();
    if ui.present { ui.max_health } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn drone_action_move() {
    log_ui_action("drone action: move");
    PENDING_MOVE_TOGGLE.store(true, Ordering::SeqCst);
}

#[unsafe(no_mangle)]
pub extern "C" fn drone_action_use() {
    log_ui_action("drone action: use");
}

#[unsafe(no_mangle)]
pub extern "C" fn move_mode_active() -> i32 {
    if MOVE_MODE_ACTIVE.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}

fn log_ui_action(label: &str) {
    #[cfg(target_arch = "wasm32")]
    miniquad::info!("{}", label);
    #[cfg(not(target_arch = "wasm32"))]
    println!("{}", label);
}

fn zoom_scale_from_power(power: i32) -> f32 {
    BASE_ZOOM_AT_POWER_ZERO * ZOOM_FACTOR.powi(power)
}

fn normalized_zoom_from_power(power: i32) -> f32 {
    ZOOM_FACTOR.powi(power)
}

fn clamp_zoom_power(power: i32) -> i32 {
    power.clamp(MIN_ZOOM_POWER, MAX_ZOOM_POWER)
}

fn take_pending_move_toggle() -> bool {
    PENDING_MOVE_TOGGLE.swap(false, Ordering::SeqCst)
}

fn set_move_mode_active(active: bool) {
    MOVE_MODE_ACTIVE.store(active, Ordering::SeqCst);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RenderChunkKey {
    chunk_x: i32,
    chunk_y: i32,
    z: i32,
}

struct RenderedLevelCache {
    z: i32,
    origin_chunk_x: i32,
    origin_chunk_y: i32,
    chunks_x: usize,
    chunks_y: usize,
    texture: Texture2D,
}

#[derive(Clone, Copy)]
struct TileRegion {
    pixel_x: u32,
    pixel_y: u32,
}

impl TileRegion {
    fn from_tile_position(position: layout::TilePosition) -> Self {
        Self {
            pixel_x: position.tile_x.saturating_mul(layout::BLOCK_PIXEL_SIZE),
            pixel_y: position.tile_y.saturating_mul(layout::BLOCK_PIXEL_SIZE),
        }
    }
}

struct BlockPalette;

impl BlockPalette {
    fn color_for(&self, block: BlockId) -> Color {
        match block {
            AIR => Color::from_rgba(0, 0, 0, 0),
            droneforge_core::DIRT => Color::from_rgba(143, 99, 63, 255),
            droneforge_core::STONE => Color::from_rgba(120, 120, 120, 255),
            droneforge_core::IRON => Color::from_rgba(194, 133, 74, 255),
            BEDROCK => Color::from_rgba(45, 45, 45, 255),
            _ => MAGENTA,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct WallTileKey {
    block: BlockId,
    mask: u8,
}

struct TileSet {
    atlas: Image,
    floor_tiles: HashMap<BlockId, TileRegion>,
    wall_tiles: HashMap<WallTileKey, TileRegion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionMode {
    Inspect,
    MoveTarget,
}

impl Default for SelectionMode {
    fn default() -> Self {
        SelectionMode::Inspect
    }
}

impl TileSet {
    async fn load_from_assets() -> Self {
        let atlas = load_image(TILESET_PATH)
            .await
            .unwrap_or_else(|err| panic!("failed to load tileset at {TILESET_PATH}: {err}"));
        Self::from_atlas(atlas)
    }

    fn from_atlas(atlas: Image) -> Self {
        let mut floor_tiles = HashMap::new();
        let mut wall_tiles = HashMap::new();

        for &block in layout::SOLID_BLOCKS.iter() {
            if let Some(position) = layout::floor_tile_position(block) {
                floor_tiles.insert(block, TileRegion::from_tile_position(position));
            }

            for mask in 0u8..(layout::WALL_MASK_VARIANTS as u8) {
                if let Some(position) = layout::wall_tile_position(block, mask) {
                    wall_tiles.insert(
                        WallTileKey { block, mask },
                        TileRegion::from_tile_position(position),
                    );
                }
            }
        }

        Self {
            atlas,
            floor_tiles,
            wall_tiles,
        }
    }

    fn floor_region(&self, block: BlockId) -> Option<&TileRegion> {
        self.floor_tiles.get(&block)
    }

    fn wall_region(&self, block: BlockId, mask: u8) -> Option<&TileRegion> {
        self.wall_tiles.get(&WallTileKey { block, mask })
    }

    fn atlas(&self) -> &Image {
        &self.atlas
    }
}

struct DroneSpriteAtlas {
    texture: Texture2D,
    regions: Vec<Rect>,
    sprite_size: f32,
    columns: u32,
}

impl DroneSpriteAtlas {
    async fn load_from_assets() -> Self {
        let atlas_image = load_image(SPRITE_ATLAS_PATH).await.unwrap_or_else(|err| {
            panic!("failed to load sprite atlas at {SPRITE_ATLAS_PATH}: {err}")
        });

        let width = atlas_image.width() as u32;
        let height = atlas_image.height() as u32;
        let expected_w = DRONE_SPRITE_COLUMNS;
        let expected_h = DRONE_SPRITE_ROWS;

        if width < expected_w || height < expected_h {
            panic!(
                "sprite atlas too small: {}x{} for {}x{} grid",
                width, height, expected_w, expected_h
            );
        }

        let cell_w = width.checked_div(expected_w).unwrap_or(0).max(1);
        let cell_h = height.checked_div(expected_h).unwrap_or(0).max(1);

        if cell_w != cell_h {
            panic!(
                "non-square sprite cells not supported: {}x{} (atlas {}x{})",
                cell_w, cell_h, width, height
            );
        }

        let expected_cell = DRONE_SPRITE_SIZE_PX + DRONE_SPRITE_PADDING_PX * 2;
        if cell_w < expected_cell {
            panic!(
                "sprite atlas cells too small: cell={} expected_at_least={}",
                cell_w, expected_cell
            );
        }

        let stride = cell_w as f32;
        let pad = DRONE_SPRITE_PADDING_PX as f32;
        let sprite_size = DRONE_SPRITE_SIZE_PX as f32;
        let texture = Texture2D::from_image(&atlas_image);
        texture.set_filter(FilterMode::Linear);

        let mut regions = Vec::new();
        for row in 0..DRONE_SPRITE_ROWS {
            for col in 0..DRONE_SPRITE_COLUMNS {
                let x = col as f32 * stride + pad;
                let y = row as f32 * stride + pad;
                regions.push(Rect::new(x, y, sprite_size, sprite_size));
            }
        }

        Self {
            texture,
            regions,
            sprite_size,
            columns: DRONE_SPRITE_COLUMNS,
        }
    }

    fn texture(&self) -> &Texture2D {
        &self.texture
    }

    fn source_rect(&self, drone_index: usize, direction_index: usize) -> Option<Rect> {
        let stride = self.columns as usize;
        let index = drone_index
            .checked_mul(stride)?
            .checked_add(direction_index)?;
        self.regions.get(index).copied()
    }

    fn sprite_size(&self) -> f32 {
        self.sprite_size
    }
}

pub struct GameState {
    world: World,
    chunk_cache: ChunkCache,
    generator: DeterministicMap,
    tiles: TileSet,
    drone_sprites: DroneSpriteAtlas,
    drone_draw: DroneDrawConfig,
    scratch_image: Image,
    rendered_level: Option<RenderedLevelCache>,
    rendered_level_dirty: bool,
    view_z: i32,
    zoom: f32,
    zoom_power: i32,
    camera_offset_x: f32,
    camera_offset_y: f32,
    camera_initialized: bool,
    last_pinch_distance: Option<f32>,
    pinch_zoom_accumulator: f32,
    last_two_finger_center: Option<Vec2>,
    last_right_drag_pos: Option<Vec2>,
    selection_mode: SelectionMode,
    selected_drone: Option<usize>,
    selected_order: Option<String>,
    render_chunk_xs: Vec<i32>,
    render_chunk_ys: Vec<i32>,
    world_chunk_xs: Vec<i32>,
    world_chunk_ys: Vec<i32>,
    world_chunk_zs: Vec<i32>,
    world_chunk_z_set: HashSet<i32>,
    chunk_cache_queue: VecDeque<ChunkPosition>,
    chunk_cache_queued: HashSet<ChunkPosition>,
    chunk_cache_frame_time_total_ms: f64,
    chunk_cache_frame_time_count: u64,
    chunk_cache_last_avg_update_time: f64,
    chunk_cache_last_reported_avg_ms: f64,
    skip_chunk_cache_processing: bool,
    fps: f32,
    fps_frame_count: u32,
    fps_last_update_time: f64,
}

impl GameState {
    pub async fn new() -> Self {
        let tiles = TileSet::load_from_assets().await;
        let drone_sprites = DroneSpriteAtlas::load_from_assets().await;
        let generator = DeterministicMap::new(42);
        let cache_capacity =
            ChunkCache::chunk_count_for_limits(HORIZONTAL_LIMIT, VERTICAL_LIMIT).max(1);
        let chunk_cache = ChunkCache::with_capacity(cache_capacity);
        Self::build(generator, chunk_cache, tiles, drone_sprites)
    }

    pub async fn new_with_cache(generator: DeterministicMap, chunk_cache: ChunkCache) -> Self {
        let tiles = TileSet::load_from_assets().await;
        let drone_sprites = DroneSpriteAtlas::load_from_assets().await;
        Self::build(generator, chunk_cache, tiles, drone_sprites)
    }

    fn build(
        generator: DeterministicMap,
        chunk_cache: ChunkCache,
        tiles: TileSet,
        drone_sprites: DroneSpriteAtlas,
    ) -> Self {
        let initial_zoom_power = 0;
        let (render_chunk_xs, render_chunk_ys) =
            render_chunk_ranges(VIEW_MIN_X, VIEW_MAX_X, VIEW_MIN_Y, VIEW_MAX_Y);
        let (world_chunk_xs, world_chunk_ys, world_chunk_zs) =
            ChunkCache::chunk_ranges_for_limits(HORIZONTAL_LIMIT, VERTICAL_LIMIT);
        let world_chunk_z_set: HashSet<i32> = world_chunk_zs.iter().copied().collect();
        let (chunk_width_px, chunk_depth_px) = render_chunk_pixel_dimensions();
        let scratch_image =
            Image::gen_image_color(chunk_width_px, chunk_depth_px, Color::from_rgba(0, 0, 0, 0));
        let mut world = World::new();
        world.set_drones(vec![DronePose::new(
            [0.0, -1.0, 0.0],
            [1.0, 0.0],
            "d1",
            10,
            10,
        )]);
        let mut game = Self {
            world,
            chunk_cache,
            generator,
            tiles,
            drone_sprites,
            drone_draw: DroneDrawConfig::default(),
            scratch_image,
            rendered_level: None,
            rendered_level_dirty: true,
            view_z: DEFAULT_VIEW_Z,
            zoom: zoom_scale_from_power(initial_zoom_power),
            zoom_power: initial_zoom_power,
            camera_offset_x: 0.0,
            camera_offset_y: 0.0,
            camera_initialized: false,
            last_pinch_distance: None,
            pinch_zoom_accumulator: 0.0,
            last_two_finger_center: None,
            last_right_drag_pos: None,
            selection_mode: SelectionMode::Inspect,
            selected_drone: None,
            selected_order: None,
            render_chunk_xs,
            render_chunk_ys,
            world_chunk_xs,
            world_chunk_ys,
            world_chunk_zs,
            world_chunk_z_set,
            chunk_cache_queue: VecDeque::new(),
            chunk_cache_queued: HashSet::new(),
            chunk_cache_frame_time_total_ms: 0.0,
            chunk_cache_frame_time_count: 0,
            chunk_cache_last_avg_update_time: 0.0,
            chunk_cache_last_reported_avg_ms: 0.0,
            skip_chunk_cache_processing: false,
            fps: 0.0,
            fps_frame_count: 0,
            fps_last_update_time: 0.0,
        };

        debug_assert!(game.drone_sprites.source_rect(0, 0).is_some());
        debug_assert!(game.drone_sprites.sprite_size() > 0.0);

        set_move_mode_active(false);

        game.prime_chunk_cache_queue();

        let now = get_time();
        game.chunk_cache_last_avg_update_time = now;
        game.fps_last_update_time = now;
        game.sync_selected_ui();
        game
    }

    fn world_to_screen(&self, world_x: i32, world_y: i32, effective_block_size: f32) -> Vec2 {
        let screen_x = (world_x - VIEW_MIN_X) as f32 * effective_block_size + self.camera_offset_x;
        let screen_y = (world_y - VIEW_MIN_Y) as f32 * effective_block_size + self.camera_offset_y;
        vec2(screen_x, screen_y)
    }

    fn world_to_screen_f(&self, world: Vec2, effective_block_size: f32) -> Vec2 {
        let screen_x = (world.x - VIEW_MIN_X as f32) * effective_block_size + self.camera_offset_x;
        let screen_y = (world.y - VIEW_MIN_Y as f32) * effective_block_size + self.camera_offset_y;
        vec2(screen_x, screen_y)
    }

    fn screen_to_world(&self, screen: Vec2, effective_block_size: f32) -> Vec3 {
        let world_x =
            ((screen.x - self.camera_offset_x) / effective_block_size) + VIEW_MIN_X as f32;
        let world_y =
            ((screen.y - self.camera_offset_y) / effective_block_size) + VIEW_MIN_Y as f32;
        Vec3::new(world_x, world_y, self.view_z as f32)
    }

    fn initialize_camera_center(&mut self) {
        if !self.camera_initialized {
            let effective_block_size = BLOCK_PIXEL_SIZE as f32 * self.zoom;
            let screen_center_x = screen_width() / 2.0;
            let screen_center_y = screen_height() / 2.0;

            // Set camera offset so world (0, 0) is at screen center
            // screen_x = (world_x - VIEW_MIN_X) * effective_block_size + camera_offset_x
            // For world (0, 0) at screen center:
            // screen_center_x = (0 - VIEW_MIN_X) * effective_block_size + camera_offset_x
            // camera_offset_x = screen_center_x - (0 - VIEW_MIN_X) * effective_block_size
            self.camera_offset_x =
                screen_center_x - (0.0 - VIEW_MIN_X as f32) * effective_block_size;
            self.camera_offset_y =
                screen_center_y - (0.0 - VIEW_MIN_Y as f32) * effective_block_size;

            self.camera_initialized = true;
        }
    }

    fn render_chunk_ready(&self, key: &RenderChunkKey) -> bool {
        let floor_chunk_z = div_floor(key.z.saturating_sub(1), CHUNK_HEIGHT as i32);
        let wall_chunk_z = div_floor(key.z, CHUNK_HEIGHT as i32);

        let base = ChunkPosition::new(key.chunk_x, key.chunk_y, floor_chunk_z);
        let wall = ChunkPosition::new(key.chunk_x, key.chunk_y, wall_chunk_z);

        self.chunk_cache.has_chunk(&base) && self.chunk_cache.has_chunk(&wall)
    }

    fn prime_chunk_cache_queue(&mut self) {
        let base_level = DEFAULT_VIEW_Z;
        let mut prioritized_levels = vec![base_level.saturating_sub(1), base_level];
        for dz in (-PRELOAD_Z_RADIUS - 1)..=PRELOAD_Z_RADIUS {
            if dz == -1 || dz == 0 {
                continue;
            }
            prioritized_levels.push(base_level.saturating_add(dz));
        }
        self.rebuild_chunk_cache_queue_from_world_levels(&prioritized_levels);
    }

    fn reprioritize_chunk_cache_for_view(&mut self, base_world_z: i32) {
        let mut prioritized_levels = Vec::new();
        prioritized_levels.push(base_world_z.saturating_sub(1));
        prioritized_levels.push(base_world_z);
        for dz in (-PRELOAD_Z_RADIUS - 1)..=PRELOAD_Z_RADIUS {
            if dz == -1 || dz == 0 {
                continue;
            }
            prioritized_levels.push(base_world_z.saturating_add(dz));
        }
        self.rebuild_chunk_cache_queue_from_world_levels(&prioritized_levels);
        self.skip_chunk_cache_processing = true;
    }

    fn rebuild_chunk_cache_queue_from_world_levels(&mut self, levels: &[i32]) {
        self.chunk_cache_queue.clear();
        self.chunk_cache_queued.clear();

        let mut ordered_chunk_zs = Vec::new();
        let mut seen_chunk_zs = HashSet::new();

        for &level in levels {
            let chunk_z = div_floor(level, CHUNK_HEIGHT as i32);
            if !self.world_chunk_z_set.contains(&chunk_z) {
                continue;
            }
            if seen_chunk_zs.insert(chunk_z) {
                ordered_chunk_zs.push(chunk_z);
            }
        }

        for &chunk_z in &self.world_chunk_zs {
            if seen_chunk_zs.contains(&chunk_z) {
                continue;
            }
            ordered_chunk_zs.push(chunk_z);
        }

        for chunk_z in ordered_chunk_zs {
            self.queue_full_chunk_plane(chunk_z);
        }
    }

    fn queue_full_chunk_plane(&mut self, chunk_z: i32) {
        let chunk_ys = self.world_chunk_ys.clone();
        let chunk_xs = self.world_chunk_xs.clone();

        for chunk_y in chunk_ys.into_iter() {
            for chunk_x in chunk_xs.iter().copied() {
                let position = ChunkPosition::new(chunk_x, chunk_y, chunk_z);
                self.queue_chunk_cache_position(position);
            }
        }
    }

    fn queue_chunk_cache_position(&mut self, position: ChunkPosition) {
        if self.chunk_cache.has_chunk(&position) || self.chunk_cache_queued.contains(&position) {
            return;
        }

        self.chunk_cache_queued.insert(position);
        self.chunk_cache_queue.push_back(position);
    }

    fn process_chunk_cache_queue(&mut self) {
        if self.skip_chunk_cache_processing {
            self.skip_chunk_cache_processing = false;
            return;
        }

        let frame_start = get_time();
        let mut processed = 0usize;
        while processed < CHUNK_CACHE_CHUNKS_PER_FRAME {
            if let Some(position) = self.chunk_cache_queue.pop_front() {
                self.chunk_cache_queued.remove(&position);
                if self.chunk_cache.has_chunk(&position) {
                    continue;
                }

                self.chunk_cache
                    .populate_chunk_at(&self.generator, position);
                processed += 1;
            } else {
                break;
            }
        }

        if processed > 0 {
            let duration_ms = (get_time() - frame_start) * 1000.0;
            self.record_chunk_cache_frame_time(duration_ms);
        }
    }

    fn record_chunk_cache_frame_time(&mut self, duration_ms: f64) {
        self.chunk_cache_frame_time_total_ms += duration_ms;
        self.chunk_cache_frame_time_count += 1;
    }

    fn average_chunk_cache_frame_time_ms(&self) -> f64 {
        if self.chunk_cache_frame_time_count == 0 {
            return 0.0;
        }
        self.chunk_cache_frame_time_total_ms / self.chunk_cache_frame_time_count as f64
    }

    fn update_chunk_cache_average_if_due(&mut self) {
        let now = get_time();
        if now - self.chunk_cache_last_avg_update_time >= LOAD_METRIC_INTERVAL_SECS {
            self.chunk_cache_last_avg_update_time = now;
            self.chunk_cache_last_reported_avg_ms = self.average_chunk_cache_frame_time_ms();
        }
    }

    fn pending_chunk_cache_counts(&self) -> (usize, usize) {
        let chunk_z = div_floor(self.view_z.saturating_sub(1), CHUNK_HEIGHT as i32);
        let wall_chunk_z = div_floor(self.view_z, CHUNK_HEIGHT as i32);

        let plane_chunks = self
            .world_chunk_xs
            .len()
            .saturating_mul(self.world_chunk_ys.len());
        let planes = if chunk_z == wall_chunk_z { 1 } else { 2 };
        let theoretical = plane_chunks.saturating_mul(planes);

        let mut pending = 0usize;
        for position in &self.chunk_cache_queue {
            if position.z == chunk_z || position.z == wall_chunk_z {
                if !self.chunk_cache.has_chunk(position) {
                    pending = pending.saturating_add(1);
                }
            }
        }

        (pending, theoretical)
    }

    fn build_level_texture(&mut self, z: i32) -> RenderedLevelCache {
        let palette = BlockPalette;
        let floor_z = z.saturating_sub(1);
        let wall_z = z;
        let mut chunk_xs = self.render_chunk_xs.clone();
        let mut chunk_ys = self.render_chunk_ys.clone();
        chunk_xs.sort_unstable();
        chunk_ys.sort_unstable();

        let origin_chunk_x = *chunk_xs.first().unwrap_or(&0);
        let origin_chunk_y = *chunk_ys.first().unwrap_or(&0);
        let chunks_x = chunk_xs.len().max(1);
        let chunks_y = chunk_ys.len().max(1);

        let (chunk_w_px, chunk_h_px) = render_chunk_pixel_dimensions();
        let chunks_x_u16 = u16::try_from(chunks_x).unwrap_or(u16::MAX);
        let chunks_y_u16 = u16::try_from(chunks_y).unwrap_or(u16::MAX);
        let level_w_px = chunk_w_px.saturating_mul(chunks_x_u16);
        let level_h_px = chunk_h_px.saturating_mul(chunks_y_u16);

        self.scratch_image =
            Image::gen_image_color(level_w_px, level_h_px, Color::from_rgba(0, 0, 0, 0));

        for (chunk_y_index, chunk_y) in chunk_ys.iter().copied().enumerate() {
            for (chunk_x_index, chunk_x) in chunk_xs.iter().copied().enumerate() {
                let key = RenderChunkKey {
                    chunk_x,
                    chunk_y,
                    z,
                };
                if !self.render_chunk_ready(&key) {
                    continue;
                }

                let base_x = chunk_x * RENDER_CHUNK_SIZE;
                let base_y = chunk_y * RENDER_CHUNK_SIZE;
                let dst_base_x = chunk_x_index.saturating_mul(RENDER_CHUNK_SIZE as usize);
                let dst_base_y = chunk_y_index.saturating_mul(RENDER_CHUNK_SIZE as usize);

                for y in 0..RENDER_CHUNK_SIZE as usize {
                    for x in 0..RENDER_CHUNK_SIZE as usize {
                        let world_x = base_x + x as i32;
                        let world_y = base_y + y as i32;
                        let block = block_at(&self.chunk_cache, world_x, world_y, floor_z);
                        let wall_block = block_at(&self.chunk_cache, world_x, world_y, wall_z);

                        let Some(block) = block else {
                            continue;
                        };

                        let dst_x = dst_base_x.saturating_add(x);
                        let dst_y = dst_base_y.saturating_add(y);

                        if let Some(wall_block) = wall_block {
                            if is_solid(wall_block) {
                                let mask =
                                    wall_edge_mask(&self.chunk_cache, world_x, world_y, wall_z);

                                if let Some(tile) = self.tiles.wall_region(wall_block, mask) {
                                    blit_tile_region(
                                        &mut self.scratch_image,
                                        self.tiles.atlas(),
                                        tile,
                                        dst_x,
                                        dst_y,
                                    );
                                } else if let Some(tile) = self.tiles.floor_region(block) {
                                    blit_tile_region(
                                        &mut self.scratch_image,
                                        self.tiles.atlas(),
                                        tile,
                                        dst_x,
                                        dst_y,
                                    );
                                } else {
                                    let color = palette.color_for(block);
                                    fill_block(&mut self.scratch_image, dst_x, dst_y, color);
                                }
                            } else if let Some(tile) = self.tiles.floor_region(block) {
                                blit_tile_region(
                                    &mut self.scratch_image,
                                    self.tiles.atlas(),
                                    tile,
                                    dst_x,
                                    dst_y,
                                );
                            } else {
                                let color = palette.color_for(block);
                                fill_block(&mut self.scratch_image, dst_x, dst_y, color);
                            }
                        } else if let Some(tile) = self.tiles.floor_region(block) {
                            blit_tile_region(
                                &mut self.scratch_image,
                                self.tiles.atlas(),
                                tile,
                                dst_x,
                                dst_y,
                            );
                        } else {
                            let color = palette.color_for(block);
                            fill_block(&mut self.scratch_image, dst_x, dst_y, color);
                        }
                    }
                }
            }
        }

        let texture = Texture2D::from_image(&self.scratch_image);
        texture.set_filter(FilterMode::Nearest);

        RenderedLevelCache {
            z,
            origin_chunk_x,
            origin_chunk_y,
            chunks_x,
            chunks_y,
            texture,
        }
    }

    fn update_fps_if_due(&mut self) {
        let now = get_time();
        self.fps_frame_count += 1;
        let elapsed = now - self.fps_last_update_time;
        if elapsed >= 1.0 {
            self.fps = self.fps_frame_count as f32 / elapsed as f32;
            self.fps_frame_count = 0;
            self.fps_last_update_time = now;
        }
    }

    fn set_view_z(&mut self, next_view_z: i32) {
        if self.view_z == next_view_z {
            return;
        }

        self.view_z = next_view_z;
        self.rendered_level = None;
        self.rendered_level_dirty = true;
        self.reprioritize_chunk_cache_for_view(next_view_z);
    }

    fn shift_view_z(&mut self, delta: i32) {
        if delta == 0 {
            return;
        }

        let next_view_z = self.view_z.saturating_add(delta);
        self.set_view_z(next_view_z);
    }

    fn fixed_update(&mut self) {
        self.world.step();
    }

    fn render(&mut self) {
        clear_background(BLACK);
        // Keep the drone sprite atlas warm and available for future draw calls.
        let _ = self.drone_sprites.texture();

        let effective_block_size = BLOCK_PIXEL_SIZE as f32 * self.zoom;
        let normalized_zoom = normalized_zoom_from_power(self.zoom_power);
        let (pending_two_levels, total_two_levels) = self.pending_chunk_cache_counts();

        if self.rendered_level_dirty && pending_two_levels == 0 {
            self.rendered_level = Some(self.build_level_texture(self.view_z));
            self.rendered_level_dirty = false;
        }

        if let Some(rendered_level) = &self.rendered_level {
            if rendered_level.z == self.view_z {
                let world_origin_x = rendered_level.origin_chunk_x * RENDER_CHUNK_SIZE;
                let world_origin_y = rendered_level.origin_chunk_y * RENDER_CHUNK_SIZE;

                let origin_screen =
                    self.world_to_screen(world_origin_x, world_origin_y, effective_block_size);

                let blocks_w = rendered_level.chunks_x as f32
                    * RENDER_CHUNK_SIZE as f32
                    * effective_block_size;
                let blocks_h = rendered_level.chunks_y as f32
                    * RENDER_CHUNK_SIZE as f32
                    * effective_block_size;

                draw_texture_ex(
                    &rendered_level.texture,
                    origin_screen.x,
                    origin_screen.y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(blocks_w, blocks_h)),
                        ..Default::default()
                    },
                );
            }
        }

        self.render_drones(effective_block_size);

        draw_text(
            &format!("tick: {}", self.world.tick),
            20.0,
            40.0,
            24.0,
            WHITE,
        );

        draw_text(
            &format!("chunk cache queue: {}", self.chunk_cache_queue.len()),
            20.0,
            64.0,
            24.0,
            WHITE,
        );

        draw_text(
            &format!(
                "chunk cache q z: {}/{}",
                pending_two_levels, total_two_levels
            ),
            20.0,
            88.0,
            24.0,
            WHITE,
        );

        let cache_avg_text = if self.chunk_cache_frame_time_count == 0 {
            "avg chunk cache: --".to_string()
        } else {
            format!(
                "avg chunk cache: {:.2} ms",
                self.chunk_cache_last_reported_avg_ms
            )
        };
        draw_text(&cache_avg_text, 20.0, 112.0, 24.0, WHITE);

        draw_text(
            &format!("zoom power: {}", self.zoom_power),
            20.0,
            136.0,
            24.0,
            WHITE,
        );

        let (mouse_x, mouse_y) = mouse_position();
        let tile_x =
            (((mouse_x - self.camera_offset_x) / effective_block_size) + VIEW_MIN_X as f32) as i32;
        let tile_y =
            (((mouse_y - self.camera_offset_y) / effective_block_size) + VIEW_MIN_Y as f32) as i32;
        let tile_z = self.view_z;

        draw_text(
            &format!("mouse: {}, {}, {}", tile_x, tile_y, tile_z),
            20.0,
            160.0,
            24.0,
            WHITE,
        );

        let screen_center_x = screen_width() / 2.0;
        let screen_center_y = screen_height() / 2.0;
        let center_tile_x = (((screen_center_x - self.camera_offset_x) / effective_block_size)
            + VIEW_MIN_X as f32) as i32;
        let center_tile_y = (((screen_center_y - self.camera_offset_y) / effective_block_size)
            + VIEW_MIN_Y as f32) as i32;

        draw_text(
            &format!(
                "center: {}, {}, {}",
                center_tile_x, center_tile_y, self.view_z
            ),
            20.0,
            184.0,
            24.0,
            WHITE,
        );

        draw_text(&format!("z: {}", self.view_z), 20.0, 208.0, 24.0, WHITE);
        draw_text(
            &format!("zoom: {:.2}x", normalized_zoom),
            20.0,
            232.0,
            24.0,
            WHITE,
        );

        draw_text(&format!("fps: {:.1}", self.fps), 20.0, 256.0, 24.0, WHITE);
    }

    fn render_drones(&self, effective_block_size: f32) {
        for drone in self.world.drones() {
            if !is_visible_at_view(drone, self.view_z) {
                continue;
            }

            let center_world = drone_world_center(drone);
            let center_screen = self.world_to_screen_f(center_world, effective_block_size);
            draw_drone(drone, center_screen, effective_block_size, &self.drone_draw);
        }
    }

    fn apply_zoom_power_at_screen_pos(&mut self, next_zoom_power: i32, focus_screen_pos: Vec2) {
        let clamped_power = clamp_zoom_power(next_zoom_power);
        if clamped_power == self.zoom_power {
            return;
        }

        let old_effective_size = BLOCK_PIXEL_SIZE as f32 * self.zoom;
        let world_x_at_focus =
            ((focus_screen_pos.x - self.camera_offset_x) / old_effective_size) + VIEW_MIN_X as f32;
        let world_y_at_focus =
            ((focus_screen_pos.y - self.camera_offset_y) / old_effective_size) + VIEW_MIN_Y as f32;

        self.zoom_power = clamped_power;
        self.zoom = zoom_scale_from_power(self.zoom_power);

        let new_effective_size = BLOCK_PIXEL_SIZE as f32 * self.zoom;
        self.camera_offset_x =
            focus_screen_pos.x - (world_x_at_focus - VIEW_MIN_X as f32) * new_effective_size;
        self.camera_offset_y =
            focus_screen_pos.y - (world_y_at_focus - VIEW_MIN_Y as f32) * new_effective_size;
    }

    fn handle_mouse_wheel_zoom(&mut self) {
        let (_wheel_x, wheel_y) = mouse_wheel();
        if wheel_y == 0.0 {
            return;
        }

        let power_delta = wheel_y.signum() as i32;
        let (mouse_x, mouse_y) = mouse_position();
        let focus = vec2(mouse_x, mouse_y);
        self.apply_zoom_power_at_screen_pos(self.zoom_power + power_delta, focus);
    }

    fn handle_pinch_zoom(&mut self) {
        let active_touches: Vec<_> = touches()
            .into_iter()
            .filter(|t| {
                matches!(
                    t.phase,
                    TouchPhase::Started | TouchPhase::Moved | TouchPhase::Stationary
                )
            })
            .collect();

        if active_touches.len() < 2 {
            self.last_pinch_distance = None;
            self.pinch_zoom_accumulator = 0.0;
            self.last_two_finger_center = None;
            return;
        }

        let first = &active_touches[0];
        let second = &active_touches[1];
        let current_distance = first.position.distance(second.position);
        if current_distance <= f32::EPSILON {
            return;
        }

        let focus = (first.position + second.position) / 2.0;

        // Two-finger pan: shift camera by the movement of the pinch center.
        if let Some(previous_center) = self.last_two_finger_center {
            let delta = focus - previous_center;
            self.camera_offset_x += delta.x;
            self.camera_offset_y += delta.y;
        }
        self.last_two_finger_center = Some(focus);

        if let Some(previous_distance) = self.last_pinch_distance {
            if previous_distance > 0.0 {
                let ratio = current_distance / previous_distance;
                if ratio > 0.0 {
                    let delta_power = ratio.ln() / ZOOM_FACTOR.ln();
                    self.pinch_zoom_accumulator += delta_power;

                    let mut applied_steps = 0;
                    while self.pinch_zoom_accumulator >= 1.0 {
                        applied_steps += 1;
                        self.pinch_zoom_accumulator -= 1.0;
                    }
                    while self.pinch_zoom_accumulator <= -1.0 {
                        applied_steps -= 1;
                        self.pinch_zoom_accumulator += 1.0;
                    }

                    if applied_steps != 0 {
                        let target_power = self.zoom_power + applied_steps;
                        self.apply_zoom_power_at_screen_pos(target_power, focus);

                        // If we hit a limit, zero out the accumulator so we do not keep pushing.
                        if self.zoom_power == MIN_ZOOM_POWER || self.zoom_power == MAX_ZOOM_POWER {
                            self.pinch_zoom_accumulator = 0.0;
                        }
                    }
                }
            }
        }

        self.last_pinch_distance = Some(current_distance);
    }

    fn apply_pending_ui_actions(&mut self) {
        if take_pending_move_toggle() {
            if self.selection_mode == SelectionMode::MoveTarget {
                self.exit_move_mode();
            } else if self.selected_drone.is_some() {
                self.selection_mode = SelectionMode::MoveTarget;
                set_move_mode_active(true);
            }
        }

        if self.selection_mode == SelectionMode::MoveTarget && self.selected_drone.is_none() {
            self.exit_move_mode();
        }
    }

    fn handle_right_mouse_drag(&mut self) {
        let is_dragging = is_mouse_button_down(MouseButton::Right);
        let (mouse_x, mouse_y) = mouse_position();
        let current = vec2(mouse_x, mouse_y);

        if is_dragging {
            if let Some(last) = self.last_right_drag_pos {
                let delta = current - last;
                self.camera_offset_x += delta.x;
                self.camera_offset_y += delta.y;
            }
            self.last_right_drag_pos = Some(current);
        } else {
            self.last_right_drag_pos = None;
        }
    }

    fn handle_left_click(&mut self) {
        if !is_mouse_button_pressed(MouseButton::Left) {
            return;
        }

        match self.selection_mode {
            SelectionMode::Inspect => self.apply_selection_click(),
            SelectionMode::MoveTarget => self.handle_move_target_click(),
        }
    }

    fn apply_selection_click(&mut self) {
        let (mouse_x, mouse_y) = mouse_position();
        let screen_pos = vec2(mouse_x, mouse_y);
        let effective_block_size = BLOCK_PIXEL_SIZE as f32 * self.zoom;
        let next_selection = self.find_drone_at_screen(screen_pos, effective_block_size);

        if self.selected_drone != next_selection {
            self.selected_drone = next_selection;
            self.selected_order = None;
            self.sync_selected_ui();
        }
    }

    fn handle_move_target_click(&mut self) {
        if self.selected_drone.is_none() {
            self.exit_move_mode();
            return;
        }

        let (mouse_x, mouse_y) = mouse_position();
        let screen_pos = vec2(mouse_x, mouse_y);
        let effective_block_size = BLOCK_PIXEL_SIZE as f32 * self.zoom;
        let target_world = self.screen_to_world(screen_pos, effective_block_size);

        let order_text = format!(
            "go to {:.1}, {:.1}, {:.1}",
            target_world.x, target_world.y, target_world.z
        );

        self.selected_order = Some(order_text);
        self.exit_move_mode();
        self.sync_selected_ui();
    }

    fn find_drone_at_screen(&self, screen_pos: Vec2, effective_block_size: f32) -> Option<usize> {
        let base_radius_px = self.drone_draw.radius_tiles * effective_block_size;
        let stroke_px = self.drone_draw.stroke_ratio * base_radius_px;
        let selection_radius = base_radius_px + stroke_px + 4.0;

        let mut closest: Option<(usize, f32)> = None;

        for (index, drone) in self.world.drones().iter().enumerate() {
            if !is_visible_at_view(drone, self.view_z) {
                continue;
            }

            let center = self.world_to_screen_f(drone_world_center(drone), effective_block_size);
            let distance = center.distance(screen_pos);
            if distance <= selection_radius {
                let replace = closest
                    .as_ref()
                    .map_or(true, |(_, best_distance)| distance < *best_distance);
                if replace {
                    closest = Some((index, distance));
                }
            }
        }

        closest.map(|(index, _)| index)
    }

    fn sync_selected_ui(&self) {
        let mut ui = selected_drone_ui().lock().unwrap();
        if let Some(selected_index) = self.selected_drone {
            if let Some(drone) = self.world.drones().get(selected_index) {
                ui.present = true;
                ui.name.clear();
                ui.name.push_str(&drone.name);
                ui.health = drone.health;
                ui.max_health = drone.max_health;
                ui.status.clear();
                if let Some(order) = &self.selected_order {
                    ui.status.push_str(order);
                }
                return;
            }
        }

        ui.present = false;
        ui.name.clear();
        ui.health = 0;
        ui.max_health = 0;
        ui.status.clear();
    }

    fn exit_move_mode(&mut self) {
        self.selection_mode = SelectionMode::Inspect;
        set_move_mode_active(false);
    }
}

fn fill_block(image: &mut Image, block_x: usize, block_y: usize, color: Color) {
    let pixel_x = block_x as u32 * BLOCK_PIXEL_SIZE as u32;
    let pixel_y = block_y as u32 * BLOCK_PIXEL_SIZE as u32;

    for dy in 0..BLOCK_PIXEL_SIZE as u32 {
        for dx in 0..BLOCK_PIXEL_SIZE as u32 {
            image.set_pixel(pixel_x + dx, pixel_y + dy, color);
        }
    }
}

fn blit_tile_region(
    dst: &mut Image,
    atlas: &Image,
    region: &TileRegion,
    tile_x: usize,
    tile_y: usize,
) {
    let size = BLOCK_PIXEL_SIZE as u32;
    let dst_x0 = tile_x as u32 * size;
    let dst_y0 = tile_y as u32 * size;

    for dy in 0..size {
        for dx in 0..size {
            let color = atlas.get_pixel(region.pixel_x + dx, region.pixel_y + dy);
            dst.set_pixel(dst_x0 + dx, dst_y0 + dy, color);
        }
    }
}

fn chunk_range(min: i32, max: i32, chunk_size: i32) -> std::ops::RangeInclusive<i32> {
    div_floor(min, chunk_size)..=div_floor(max, chunk_size)
}

fn render_chunk_ranges(min_x: i32, max_x: i32, min_y: i32, max_y: i32) -> (Vec<i32>, Vec<i32>) {
    let xs: Vec<i32> = chunk_range(min_x, max_x, RENDER_CHUNK_SIZE).collect();
    let ys: Vec<i32> = chunk_range(min_y, max_y, RENDER_CHUNK_SIZE).collect();
    (xs, ys)
}

fn render_chunk_pixel_dimensions() -> (u16, u16) {
    let chunk_size_px = RENDER_CHUNK_SIZE as u16 * BLOCK_PIXEL_SIZE;
    (chunk_size_px, chunk_size_px)
}

fn div_floor(a: i32, b: i32) -> i32 {
    let (d, r) = (a / b, a % b);
    if r != 0 && ((r < 0) != (b < 0)) {
        d - 1
    } else {
        d
    }
}

fn block_at(cache: &ChunkCache, x: i32, y: i32, z: i32) -> Option<BlockId> {
    let coord = WorldCoord::new(x, y, z);
    cache.block_at_world(coord)
}

fn is_solid(block: BlockId) -> bool {
    block != AIR
}

fn is_solid_opt(block: Option<BlockId>) -> bool {
    block.map_or(false, is_solid)
}

fn wall_edge_mask(cache: &ChunkCache, x: i32, y: i32, z: i32) -> u8 {
    let mut mask = 0u8;

    if !is_solid_opt(block_at(cache, x, y - 1, z)) {
        mask |= MASK_NORTH;
    }
    if !is_solid_opt(block_at(cache, x + 1, y, z)) {
        mask |= MASK_EAST;
    }
    if !is_solid_opt(block_at(cache, x, y + 1, z)) {
        mask |= MASK_SOUTH;
    }
    if !is_solid_opt(block_at(cache, x - 1, y, z)) {
        mask |= MASK_WEST;
    }

    mask
}

pub async fn run() {
    install_panic_hook();
    let generator = DeterministicMap::new(42);
    let cache_capacity =
        ChunkCache::chunk_count_for_limits(HORIZONTAL_LIMIT, VERTICAL_LIMIT).max(1);
    let chunk_cache = ChunkCache::with_capacity(cache_capacity);
    let mut game = GameState::new_with_cache(generator, chunk_cache).await;
    game.initialize_camera_center();
    let mut accumulator = 0.0_f32;
    let fixed_step = 1.0_f32 / 60.0_f32; // 60 Hz simulation

    loop {
        // Consume real elapsed time in fixed-size simulation steps.
        accumulator += get_frame_time();
        while accumulator >= fixed_step {
            game.fixed_update();
            accumulator -= fixed_step;
        }

        let pending_z_delta = take_pending_z_delta();
        if pending_z_delta != 0 {
            game.shift_view_z(pending_z_delta);
        }

        game.apply_pending_ui_actions();
        game.handle_mouse_wheel_zoom();
        game.handle_pinch_zoom();
        game.handle_right_mouse_drag();
        game.handle_left_click();
        game.process_chunk_cache_queue();
        game.update_chunk_cache_average_if_due();
        game.update_fps_if_due();

        game.render();

        next_frame().await;
    }
}

#[cfg(target_arch = "wasm32")]
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        if let Some(location) = info.location() {
            miniquad::error!("panic at {}:{}: {}", location.file(), location.line(), msg);
        } else {
            miniquad::error!("panic: {}", msg);
        }
    }));
}

#[cfg(not(target_arch = "wasm32"))]
fn install_panic_hook() {}
