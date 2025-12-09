use droneforge_core::worldgen::{DeterministicMap, HORIZONTAL_LIMIT, VERTICAL_LIMIT};
use droneforge_core::{AIR, BEDROCK, BlockId, ChunkCache, ChunkPosition, World, WorldCoord};
#[cfg(target_arch = "wasm32")]
use macroquad::miniquad;
use macroquad::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

const VIEW_MIN_X: i32 = -100;
const VIEW_MAX_X: i32 = 100;
const VIEW_MIN_Y: i32 = -60;
const VIEW_MAX_Y: i32 = 60;
const DEFAULT_VIEW_Z: i32 = 0;

const BLOCK_PIXEL_SIZE: u16 = 32;
// Keep effective block size at power 0 the same as before (≈15 px from 4 px * 3.8) even with smaller sprites.
const BASE_ZOOM_AT_POWER_ZERO: f32 = 0.475;
const MIN_ZOOM_POWER: i32 = -48;
const MAX_ZOOM_POWER: i32 = 15;
const ZOOM_FACTOR: f32 = 1.1;

const RENDER_CHUNK_SIZE: i32 = 32;
const PRELOAD_Z_RADIUS: i32 = 5;
const CHUNKS_PER_FRAME: usize = 1;
const INITIAL_CACHE_CHUNKS_PER_FRAME: usize = 256;
const LOAD_METRIC_INTERVAL_SECS: f64 = 5.0;
const CHUNK_PROGRESS_STEP: u32 = 1_000;

static PENDING_Z_DELTA: AtomicI32 = AtomicI32::new(0);

static INITIAL_CHUNK_TOTAL: AtomicU32 = AtomicU32::new(0);
static INITIAL_CHUNK_LOADED: AtomicU32 = AtomicU32::new(0);

const MASK_NORTH: u8 = 1;
const MASK_EAST: u8 = 2;
const MASK_SOUTH: u8 = 4;
const MASK_WEST: u8 = 8;

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

fn reset_initial_chunk_progress(total_chunks: u32) {
    INITIAL_CHUNK_TOTAL.store(total_chunks, Ordering::SeqCst);
    INITIAL_CHUNK_LOADED.store(0, Ordering::SeqCst);
}

fn set_initial_chunk_loaded(loaded: u32) {
    INITIAL_CHUNK_LOADED.store(loaded, Ordering::SeqCst);
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

fn zoom_scale_from_power(power: i32) -> f32 {
    BASE_ZOOM_AT_POWER_ZERO * ZOOM_FACTOR.powi(power)
}

fn normalized_zoom_from_power(power: i32) -> f32 {
    ZOOM_FACTOR.powi(power)
}

fn clamp_zoom_power(power: i32) -> i32 {
    power.clamp(MIN_ZOOM_POWER, MAX_ZOOM_POWER)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RenderChunkKey {
    chunk_x: i32,
    chunk_y: i32,
    z: i32,
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
    floor_tiles: HashMap<BlockId, Image>,
    wall_tiles: HashMap<WallTileKey, Image>,
}

impl TileSet {
    fn new(palette: &BlockPalette) -> Self {
        let mut floor_tiles = HashMap::new();
        let mut wall_tiles = HashMap::new();

        let solid_blocks: &[BlockId] = &[
            droneforge_core::DIRT,
            droneforge_core::STONE,
            droneforge_core::IRON,
            BEDROCK,
        ];

        for &block in solid_blocks {
            let mut img = Image::gen_image_color(
                BLOCK_PIXEL_SIZE.into(),
                BLOCK_PIXEL_SIZE.into(),
                Color::from_rgba(0, 0, 0, 0),
            );
            fill_block(&mut img, 0, 0, palette.color_for(block));
            floor_tiles.insert(block, img);
        }

        for &block in solid_blocks {
            let source_color = palette.color_for(block);
            let base_color = wall_base_tint(source_color);
            let edge_color = wall_edge_tint(source_color);

            for mask in 0u8..16 {
                let mut img = Image::gen_image_color(
                    BLOCK_PIXEL_SIZE.into(),
                    BLOCK_PIXEL_SIZE.into(),
                    Color::from_rgba(0, 0, 0, 0),
                );
                draw_wall_overlay(&mut img, 0, 0, base_color, edge_color, mask);
                draw_wall_outline(&mut img, 0, 0, mask);
                wall_tiles.insert(WallTileKey { block, mask }, img);
            }
        }

        Self {
            floor_tiles,
            wall_tiles,
        }
    }

    fn floor_image(&self, block: BlockId) -> Option<&Image> {
        self.floor_tiles.get(&block)
    }

    fn wall_image(&self, block: BlockId, mask: u8) -> Option<&Image> {
        self.wall_tiles.get(&WallTileKey { block, mask })
    }
}

pub struct GameState {
    world: World,
    chunk_cache: ChunkCache,
    render_cache: HashMap<RenderChunkKey, Texture2D>,
    load_queue: VecDeque<RenderChunkKey>,
    queued_keys: HashSet<RenderChunkKey>,
    generator: DeterministicMap,
    tiles: TileSet,
    scratch_image: Image,
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
    render_chunk_xs: Vec<i32>,
    render_chunk_ys: Vec<i32>,
    load_time_total_ms: f64,
    load_time_count: u64,
    last_avg_update_time: f64,
    last_reported_avg_ms: f64,
    initial_chunk_cache_ms: f64,
    initial_render_cache_ms: f64,
}

impl GameState {
    pub fn new() -> Self {
        let generator = DeterministicMap::new(42);
        let chunk_cache_start = get_time();
        let chunk_cache = ChunkCache::from_generator_with_limits(&generator);
        let initial_chunk_cache_ms = (get_time() - chunk_cache_start) * 1000.0;
        Self::build(generator, chunk_cache, initial_chunk_cache_ms)
    }

    pub fn new_with_cache(
        generator: DeterministicMap,
        chunk_cache: ChunkCache,
        initial_chunk_cache_ms: f64,
    ) -> Self {
        Self::build(generator, chunk_cache, initial_chunk_cache_ms)
    }

    fn build(
        generator: DeterministicMap,
        chunk_cache: ChunkCache,
        initial_chunk_cache_ms: f64,
    ) -> Self {
        let initial_zoom_power = 0;
        let palette = BlockPalette;
        let (render_chunk_xs, render_chunk_ys) =
            render_chunk_ranges(VIEW_MIN_X, VIEW_MAX_X, VIEW_MIN_Y, VIEW_MAX_Y);
        let (chunk_width_px, chunk_depth_px) = render_chunk_pixel_dimensions();
        let scratch_image =
            Image::gen_image_color(chunk_width_px, chunk_depth_px, Color::from_rgba(0, 0, 0, 0));
        let tiles = TileSet::new(&palette);
        let mut game = Self {
            world: World::new(),
            chunk_cache,
            render_cache: HashMap::new(),
            load_queue: VecDeque::new(),
            queued_keys: HashSet::new(),
            generator,
            tiles,
            scratch_image,
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
            render_chunk_xs,
            render_chunk_ys,
            load_time_total_ms: 0.0,
            load_time_count: 0,
            last_avg_update_time: 0.0,
            last_reported_avg_ms: 0.0,
            initial_chunk_cache_ms,
            initial_render_cache_ms: 0.0,
        };

        let render_cache_start = get_time();
        game.cache_levels_range_now(DEFAULT_VIEW_Z, PRELOAD_Z_RADIUS);
        game.initial_render_cache_ms = (get_time() - render_cache_start) * 1000.0;

        game.enqueue_surrounding_levels(DEFAULT_VIEW_Z);
        game.last_reported_avg_ms = game.average_load_time_ms();
        game.last_avg_update_time = get_time();
        game
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

    fn cache_level_now(&mut self, z: i32) {
        let keys: Vec<RenderChunkKey> = self
            .render_chunk_ys
            .iter()
            .flat_map(|&chunk_y| {
                self.render_chunk_xs
                    .iter()
                    .map(move |&chunk_x| RenderChunkKey {
                        chunk_x,
                        chunk_y,
                        z,
                    })
            })
            .collect();

        for key in keys {
            if self.render_cache.contains_key(&key) {
                self.queued_keys.remove(&key);
                continue;
            }

            let (texture, duration_ms) = self.build_render_chunk_texture(key);
            self.render_cache.insert(key, texture);
            self.record_load_time(duration_ms);
            self.queued_keys.remove(&key);
        }
    }

    fn cache_levels_range_now(&mut self, center_z: i32, radius: i32) {
        for dz in -radius..=radius {
            let target_z = center_z.saturating_add(dz);
            self.cache_level_now(target_z);
        }
    }

    fn enqueue_surrounding_levels(&mut self, center_z: i32) {
        for dz in -PRELOAD_Z_RADIUS..=PRELOAD_Z_RADIUS {
            if dz == 0 {
                continue;
            }

            let target_z = center_z.saturating_add(dz);
            self.enqueue_chunks_for_z(target_z);
        }
    }

    fn enqueue_chunks_for_z(&mut self, z: i32) {
        let keys: Vec<RenderChunkKey> = self
            .render_chunk_ys
            .iter()
            .flat_map(|&chunk_y| {
                self.render_chunk_xs
                    .iter()
                    .map(move |&chunk_x| RenderChunkKey {
                        chunk_x,
                        chunk_y,
                        z,
                    })
            })
            .collect();

        for key in keys {
            self.queue_key(key, false);
        }
    }

    fn prune_levels_outside_radius(&mut self, center_z: i32) {
        let min_z = center_z.saturating_sub(PRELOAD_Z_RADIUS);
        let max_z = center_z.saturating_add(PRELOAD_Z_RADIUS);
        let in_range = |z: i32| z >= min_z && z <= max_z;

        self.render_cache.retain(|key, _| in_range(key.z));
        self.queued_keys.retain(|key| in_range(key.z));
        self.load_queue.retain(|key| in_range(key.z));
    }

    fn queue_key(&mut self, key: RenderChunkKey, front: bool) {
        if self.render_cache.contains_key(&key) || self.queued_keys.contains(&key) {
            return;
        }

        self.queued_keys.insert(key);
        if front {
            self.load_queue.push_front(key);
        } else {
            self.load_queue.push_back(key);
        }
    }

    fn process_load_queue(&mut self) {
        for _ in 0..CHUNKS_PER_FRAME {
            if let Some(key) = self.load_queue.pop_front() {
                self.queued_keys.remove(&key);
                if self.render_cache.contains_key(&key) {
                    continue;
                }

                let (texture, duration_ms) = self.build_render_chunk_texture(key);
                self.render_cache.insert(key, texture);
                self.record_load_time(duration_ms);
            } else {
                break;
            }
        }
    }

    fn build_render_chunk_texture(&mut self, key: RenderChunkKey) -> (Texture2D, f64) {
        let start_secs = get_time();
        let palette = BlockPalette;

        let base_x = key.chunk_x * RENDER_CHUNK_SIZE;
        let base_y = key.chunk_y * RENDER_CHUNK_SIZE;
        let z_wall = key.z + 1;

        let (chunk_w_px, chunk_h_px) = render_chunk_pixel_dimensions();
        self.scratch_image =
            Image::gen_image_color(chunk_w_px, chunk_h_px, Color::from_rgba(0, 0, 0, 0));

        for y in 0..RENDER_CHUNK_SIZE as usize {
            for x in 0..RENDER_CHUNK_SIZE as usize {
                let world_x = base_x + x as i32;
                let world_y = base_y + y as i32;
                let block = block_at(&self.chunk_cache, &self.generator, world_x, world_y, key.z);
                let upper_block =
                    block_at(&self.chunk_cache, &self.generator, world_x, world_y, z_wall);

                if is_solid(upper_block) {
                    let mask = wall_edge_mask(
                        &self.chunk_cache,
                        &self.generator,
                        world_x,
                        world_y,
                        z_wall,
                    );

                    if let Some(tile) = self.tiles.wall_image(upper_block, mask) {
                        blit_tile(&mut self.scratch_image, tile, x, y);
                    } else if let Some(tile) = self.tiles.floor_image(block) {
                        blit_tile(&mut self.scratch_image, tile, x, y);
                    } else {
                        let color = palette.color_for(block);
                        fill_block(&mut self.scratch_image, x, y, color);
                    }
                } else if let Some(tile) = self.tiles.floor_image(block) {
                    blit_tile(&mut self.scratch_image, tile, x, y);
                } else {
                    let color = palette.color_for(block);
                    fill_block(&mut self.scratch_image, x, y, color);
                }
            }
        }

        let texture = Texture2D::from_image(&self.scratch_image);
        texture.set_filter(FilterMode::Nearest);
        let duration_ms = (get_time() - start_secs) * 1000.0;
        (texture, duration_ms)
    }

    fn record_load_time(&mut self, duration_ms: f64) {
        self.load_time_total_ms += duration_ms;
        self.load_time_count += 1;
    }

    fn average_load_time_ms(&self) -> f64 {
        if self.load_time_count == 0 {
            return 0.0;
        }
        self.load_time_total_ms / self.load_time_count as f64
    }

    fn update_average_display_if_due(&mut self) {
        let now = get_time();
        if now - self.last_avg_update_time >= LOAD_METRIC_INTERVAL_SECS {
            self.last_avg_update_time = now;
            self.last_reported_avg_ms = self.average_load_time_ms();
        }
    }

    fn set_view_z(&mut self, next_view_z: i32) {
        if self.view_z == next_view_z {
            return;
        }

        self.view_z = next_view_z;
        self.cache_level_now(next_view_z);
        self.enqueue_surrounding_levels(next_view_z);
        self.prune_levels_outside_radius(next_view_z);
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

    fn render(&self) {
        clear_background(BLACK);

        let effective_block_size = BLOCK_PIXEL_SIZE as f32 * self.zoom;
        let normalized_zoom = normalized_zoom_from_power(self.zoom_power);
        let chunk_pixel_size = vec2(
            RENDER_CHUNK_SIZE as f32 * effective_block_size,
            RENDER_CHUNK_SIZE as f32 * effective_block_size,
        );

        for &chunk_y in &self.render_chunk_ys {
            for &chunk_x in &self.render_chunk_xs {
                let key = RenderChunkKey {
                    chunk_x,
                    chunk_y,
                    z: self.view_z,
                };

                if let Some(texture) = self.render_cache.get(&key) {
                    let world_origin_x = chunk_x * RENDER_CHUNK_SIZE;
                    let world_origin_y = chunk_y * RENDER_CHUNK_SIZE;

                    let screen_x = (world_origin_x - VIEW_MIN_X) as f32 * effective_block_size
                        + self.camera_offset_x;
                    let screen_y = (world_origin_y - VIEW_MIN_Y) as f32 * effective_block_size
                        + self.camera_offset_y;

                    draw_texture_ex(
                        texture,
                        screen_x,
                        screen_y,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(chunk_pixel_size),
                            ..Default::default()
                        },
                    );
                }
            }
        }

        draw_text(
            &format!("tick: {}", self.world.tick),
            20.0,
            40.0,
            24.0,
            WHITE,
        );

        draw_text(
            &format!("chunk cache: {:.2} ms", self.initial_chunk_cache_ms),
            20.0,
            64.0,
            24.0,
            WHITE,
        );

        draw_text(
            &format!("render cache ±5: {:.2} ms", self.initial_render_cache_ms),
            20.0,
            88.0,
            24.0,
            WHITE,
        );

        let avg_text = if self.load_time_count == 0 {
            "avg chunk load: --".to_string()
        } else {
            format!("avg chunk load: {:.2} ms", self.last_reported_avg_ms)
        };
        draw_text(&avg_text, 20.0, 112.0, 24.0, WHITE);

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
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
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

fn blit_tile(dst: &mut Image, tile: &Image, tile_x: usize, tile_y: usize) {
    let size = BLOCK_PIXEL_SIZE as u32;
    let dst_x0 = tile_x as u32 * size;
    let dst_y0 = tile_y as u32 * size;

    for dy in 0..size {
        for dx in 0..size {
            let color = tile.get_pixel(dx, dy);
            dst.set_pixel(dst_x0 + dx, dst_y0 + dy, color);
        }
    }
}

fn fill_rect(image: &mut Image, start_x: u32, start_y: u32, width: u32, height: u32, color: Color) {
    for dy in 0..height {
        for dx in 0..width {
            image.set_pixel(start_x + dx, start_y + dy, color);
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

fn block_at(cache: &ChunkCache, generator: &DeterministicMap, x: i32, y: i32, z: i32) -> BlockId {
    let coord = WorldCoord::new(x, y, z);
    cache
        .block_at_world(coord)
        .unwrap_or_else(|| generator.block_at(coord))
}

fn is_solid(block: BlockId) -> bool {
    block != AIR
}

fn wall_edge_mask(cache: &ChunkCache, generator: &DeterministicMap, x: i32, y: i32, z: i32) -> u8 {
    let mut mask = 0u8;

    if !is_solid(block_at(cache, generator, x, y - 1, z)) {
        mask |= MASK_NORTH;
    }
    if !is_solid(block_at(cache, generator, x + 1, y, z)) {
        mask |= MASK_EAST;
    }
    if !is_solid(block_at(cache, generator, x, y + 1, z)) {
        mask |= MASK_SOUTH;
    }
    if !is_solid(block_at(cache, generator, x - 1, y, z)) {
        mask |= MASK_WEST;
    }

    mask
}

fn apply_saturation_and_brightness(color: Color, saturation: f32, brightness: f32) -> Color {
    let intensity = (color.r + color.g + color.b) / 3.0;
    let adjust = |channel: f32| -> f32 {
        let saturated = intensity + (channel - intensity) * saturation;
        (saturated * brightness).clamp(0.0, 1.0)
    };

    Color {
        r: adjust(color.r),
        g: adjust(color.g),
        b: adjust(color.b),
        a: color.a,
    }
}

fn wall_base_tint(color: Color) -> Color {
    apply_saturation_and_brightness(color, 1.25, 0.2)
}

fn wall_edge_tint(color: Color) -> Color {
    apply_saturation_and_brightness(color, 1.2, 1.15)
}

fn wall_outline_thickness(size: u32) -> u32 {
    (size / 16).max(1)
}

fn wall_rim_thickness(size: u32) -> u32 {
    // Keep rim + outline at 1/4 of the block, with outline outermost.
    let target_total = (size / 4).max(1);
    let outline = wall_outline_thickness(size);
    target_total.saturating_sub(outline).max(1)
}

fn draw_wall_overlay(
    image: &mut Image,
    block_x: usize,
    block_y: usize,
    base_color: Color,
    edge_color: Color,
    mask: u8,
) {
    fill_block(image, block_x, block_y, base_color);

    let size = BLOCK_PIXEL_SIZE as u32;
    let start_x = block_x as u32 * size;
    let start_y = block_y as u32 * size;
    let outline_thickness = wall_outline_thickness(size);
    let rim_thickness = wall_rim_thickness(size); // rim + outline = 1/4 block

    // Keep the bright rim just inside the outermost outline so black can be outermost.
    // For north/south we inset by the outline thickness on Y; for east/west on X.
    let north_y = start_y + outline_thickness;
    let south_y = start_y + size.saturating_sub(outline_thickness + rim_thickness);
    let west_x = start_x + outline_thickness;
    let east_x = start_x + size.saturating_sub(outline_thickness + rim_thickness);

    if mask & MASK_NORTH != 0 {
        fill_rect(image, start_x, north_y, size, rim_thickness, edge_color);
    }

    if mask & MASK_EAST != 0 {
        fill_rect(image, east_x, start_y, rim_thickness, size, edge_color);
    }

    if mask & MASK_SOUTH != 0 {
        fill_rect(image, start_x, south_y, size, rim_thickness, edge_color);
    }

    if mask & MASK_WEST != 0 {
        fill_rect(image, west_x, start_y, rim_thickness, size, edge_color);
    }
}

fn draw_wall_outline(image: &mut Image, block_x: usize, block_y: usize, mask: u8) {
    if mask == 0 {
        return;
    }

    let size = BLOCK_PIXEL_SIZE as u32;
    // Outer black outline thickness: 1/16 of block (same fraction as before), min 1px.
    let outline_thickness = wall_outline_thickness(size);
    let start_x = block_x as u32 * size;
    let start_y = block_y as u32 * size;

    if mask & MASK_NORTH != 0 {
        fill_rect(image, start_x, start_y, size, outline_thickness, BLACK);
    }

    if mask & MASK_EAST != 0 {
        fill_rect(
            image,
            start_x + size.saturating_sub(outline_thickness),
            start_y,
            outline_thickness,
            size,
            BLACK,
        );
    }

    if mask & MASK_SOUTH != 0 {
        fill_rect(
            image,
            start_x,
            start_y + size.saturating_sub(outline_thickness),
            size,
            outline_thickness,
            BLACK,
        );
    }

    if mask & MASK_WEST != 0 {
        fill_rect(image, start_x, start_y, outline_thickness, size, BLACK);
    }

    if mask & MASK_NORTH != 0 && mask & MASK_WEST != 0 {
        fill_rect(
            image,
            start_x,
            start_y,
            outline_thickness,
            outline_thickness,
            BLACK,
        );
    }

    if mask & MASK_NORTH != 0 && mask & MASK_EAST != 0 {
        fill_rect(
            image,
            start_x + size.saturating_sub(outline_thickness),
            start_y,
            outline_thickness,
            outline_thickness,
            BLACK,
        );
    }

    if mask & MASK_SOUTH != 0 && mask & MASK_WEST != 0 {
        fill_rect(
            image,
            start_x,
            start_y + size.saturating_sub(outline_thickness),
            outline_thickness,
            outline_thickness,
            BLACK,
        );
    }

    if mask & MASK_SOUTH != 0 && mask & MASK_EAST != 0 {
        fill_rect(
            image,
            start_x + size.saturating_sub(outline_thickness),
            start_y + size.saturating_sub(outline_thickness),
            outline_thickness,
            outline_thickness,
            BLACK,
        );
    }
}

async fn build_chunk_cache_with_progress(generator: &DeterministicMap) -> (ChunkCache, f64) {
    let build_start = get_time();
    let (chunk_xs, chunk_ys, chunk_zs) =
        ChunkCache::chunk_ranges_for_limits(HORIZONTAL_LIMIT, VERTICAL_LIMIT);
    let total_chunks = (chunk_xs.len() * chunk_ys.len() * chunk_zs.len()) as u32;

    reset_initial_chunk_progress(total_chunks);

    let mut chunk_cache = ChunkCache::new();
    let mut loaded_chunks: u32 = 0;
    let mut next_progress_update = CHUNK_PROGRESS_STEP.min(total_chunks.max(1));
    let mut batch_counter: usize = 0;

    for &chunk_x in &chunk_xs {
        for &chunk_y in &chunk_ys {
            for &chunk_z in &chunk_zs {
                let position = ChunkPosition::new(chunk_x, chunk_y, chunk_z);
                chunk_cache.populate_chunk_at(generator, position);
                loaded_chunks = loaded_chunks.saturating_add(1);
                batch_counter += 1;

                if loaded_chunks >= next_progress_update || loaded_chunks == total_chunks {
                    set_initial_chunk_loaded(loaded_chunks.min(total_chunks));
                    next_progress_update = (loaded_chunks.saturating_add(CHUNK_PROGRESS_STEP))
                        .min(total_chunks.max(1));
                }

                if batch_counter >= INITIAL_CACHE_CHUNKS_PER_FRAME {
                    batch_counter = 0;
                    next_frame().await;
                }
            }
        }
    }

    set_initial_chunk_loaded(total_chunks);
    let elapsed_ms = (get_time() - build_start) * 1000.0;
    (chunk_cache, elapsed_ms)
}

pub async fn run() {
    install_panic_hook();
    let generator = DeterministicMap::new(42);
    let (chunk_cache, chunk_cache_ms) = build_chunk_cache_with_progress(&generator).await;
    let mut game = GameState::new_with_cache(generator, chunk_cache, chunk_cache_ms);
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

        game.handle_mouse_wheel_zoom();
        game.handle_pinch_zoom();
        game.handle_right_mouse_drag();
        game.process_load_queue();
        game.update_average_display_if_due();

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
