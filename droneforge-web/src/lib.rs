use droneforge_core::worldgen::DeterministicMap;
use droneforge_core::{
    AIR, BEDROCK, BlockId, CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, ChunkPosition, LocalBlockCoord,
    World,
};
use macroquad::prelude::*;
use std::sync::atomic::{AtomicI32, Ordering};

const VIEW_MIN_X: i32 = -100;
const VIEW_MAX_X: i32 = 100;
const VIEW_MIN_Y: i32 = -60;
const VIEW_MAX_Y: i32 = 60;
const DEFAULT_VIEW_Z: i32 = 0;

const BLOCK_PIXEL_SIZE: u16 = 4;
const MIN_ZOOM: f32 = 0.20;
const MAX_ZOOM: f32 = 4.0;
const ZOOM_FACTOR: f32 = 1.1;

static PENDING_Z_DELTA: AtomicI32 = AtomicI32::new(0);

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

struct ChunkTexture {
    position: ChunkPosition,
    texture: Texture2D,
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

pub struct GameState {
    world: World,
    chunk_textures: Vec<ChunkTexture>,
    generator: DeterministicMap,
    view_z: i32,
    zoom: f32,
    camera_offset_x: f32,
    camera_offset_y: f32,
    camera_initialized: bool,
}

impl GameState {
    pub fn new() -> Self {
        let generator = DeterministicMap::new(42);
        let mut game = Self {
            world: World::new(),
            chunk_textures: Vec::new(),
            generator,
            view_z: DEFAULT_VIEW_Z,
            zoom: 1.0,
            camera_offset_x: 0.0,
            camera_offset_y: 0.0,
            camera_initialized: false,
        };

        game.rebuild_chunk_textures();
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
            self.camera_offset_x = screen_center_x - (0.0 - VIEW_MIN_X as f32) * effective_block_size;
            self.camera_offset_y = screen_center_y - (0.0 - VIEW_MIN_Y as f32) * effective_block_size;
            
            self.camera_initialized = true;
        }
    }

    fn rebuild_chunk_textures(&mut self) {
        self.chunk_textures.clear();

        let palette = BlockPalette;
        let chunk_z = div_floor(self.view_z, CHUNK_HEIGHT as i32);

        for chunk_y in chunk_range(VIEW_MIN_Y, VIEW_MAX_Y, CHUNK_DEPTH as i32) {
            for chunk_x in chunk_range(VIEW_MIN_X, VIEW_MAX_X, CHUNK_WIDTH as i32) {
                let position = ChunkPosition::new(chunk_x, chunk_y, chunk_z);
                self.world
                    .register_generated_chunk(position, &self.generator);

                if let Some(chunk) = self.world.chunk(&position) {
                    let texture = build_chunk_texture(chunk, self.view_z, &palette);
                    self.chunk_textures.push(ChunkTexture { position, texture });
                }
            }
        }
    }

    fn set_view_z(&mut self, next_view_z: i32) {
        if self.view_z == next_view_z {
            return;
        }

        self.view_z = next_view_z;
        self.rebuild_chunk_textures();
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
        let chunk_pixel_size = vec2(
            CHUNK_WIDTH as f32 * effective_block_size,
            CHUNK_DEPTH as f32 * effective_block_size,
        );

        for chunk_texture in &self.chunk_textures {
            let world_origin_x = chunk_texture.position.x * CHUNK_WIDTH as i32;
            let world_origin_y = chunk_texture.position.y * CHUNK_DEPTH as i32;

            let screen_x = (world_origin_x - VIEW_MIN_X) as f32 * effective_block_size + self.camera_offset_x;
            let screen_y = (world_origin_y - VIEW_MIN_Y) as f32 * effective_block_size + self.camera_offset_y;

            draw_texture_ex(
                &chunk_texture.texture,
                screen_x,
                screen_y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(chunk_pixel_size),
                    ..Default::default()
                },
            );
        }

        draw_text(
            &format!("tick: {}", self.world.tick),
            20.0,
            40.0,
            24.0,
            WHITE,
        );

        let (mouse_x, mouse_y) = mouse_position();
        let tile_x = (((mouse_x - self.camera_offset_x) / effective_block_size) + VIEW_MIN_X as f32) as i32;
        let tile_y = (((mouse_y - self.camera_offset_y) / effective_block_size) + VIEW_MIN_Y as f32) as i32;
        let tile_z = self.view_z;

        draw_text(
            &format!("mouse: {}, {}, {}", tile_x, tile_y, tile_z),
            20.0,
            68.0,
            24.0,
            WHITE,
        );

        let screen_center_x = screen_width() / 2.0;
        let screen_center_y = screen_height() / 2.0;
        let center_tile_x = (((screen_center_x - self.camera_offset_x) / effective_block_size) + VIEW_MIN_X as f32) as i32;
        let center_tile_y = (((screen_center_y - self.camera_offset_y) / effective_block_size) + VIEW_MIN_Y as f32) as i32;

        draw_text(
            &format!("center: {}, {}, {}", center_tile_x, center_tile_y, self.view_z),
            20.0,
            96.0,
            24.0,
            WHITE,
        );

        draw_text(&format!("z: {}", self.view_z), 20.0, 124.0, 24.0, WHITE);
        draw_text(&format!("zoom: {:.2}x", self.zoom), 20.0, 152.0, 24.0, WHITE);
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

fn build_chunk_texture(
    chunk: &droneforge_core::Chunk,
    world_z: i32,
    palette: &BlockPalette,
) -> Texture2D {
    let chunk_width_px = CHUNK_WIDTH as u16 * BLOCK_PIXEL_SIZE;
    let chunk_depth_px = CHUNK_DEPTH as u16 * BLOCK_PIXEL_SIZE;
    let mut image =
        Image::gen_image_color(chunk_width_px, chunk_depth_px, Color::from_rgba(0, 0, 0, 0));

    let chunk_base_z = chunk.position.z * CHUNK_HEIGHT as i32;
    let local_z = (world_z - chunk_base_z) as usize;

    debug_assert!(local_z < CHUNK_HEIGHT);

    for y in 0..CHUNK_DEPTH {
        for x in 0..CHUNK_WIDTH {
            let block = chunk
                .get_block(LocalBlockCoord::new(x, y, local_z))
                .unwrap_or(AIR);
            let color = palette.color_for(block);
            fill_block(&mut image, x, y, color);
        }
    }

    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Nearest);
    texture
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

fn chunk_range(min: i32, max: i32, chunk_size: i32) -> std::ops::RangeInclusive<i32> {
    div_floor(min, chunk_size)..=div_floor(max, chunk_size)
}

fn div_floor(a: i32, b: i32) -> i32 {
    let (d, r) = (a / b, a % b);
    if r != 0 && ((r < 0) != (b < 0)) {
        d - 1
    } else {
        d
    }
}

pub async fn run() {
    let mut game = GameState::new();
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

        // Handle mouse wheel zoom around screen center
        let (_wheel_x, wheel_y) = mouse_wheel();
        if wheel_y != 0.0 {
            let old_zoom = game.zoom;
            let new_zoom = game.zoom * (ZOOM_FACTOR.powf(wheel_y.signum()));
            if new_zoom != old_zoom {
                // Calculate screen center
                let screen_center_x = screen_width() / 2.0;
                let screen_center_y = screen_height() / 2.0;
                
                // Calculate world position at screen center before zoom
                let old_effective_size = BLOCK_PIXEL_SIZE as f32 * old_zoom;
                let world_x_at_center = ((screen_center_x - game.camera_offset_x) / old_effective_size) + VIEW_MIN_X as f32;
                let world_y_at_center = ((screen_center_y - game.camera_offset_y) / old_effective_size) + VIEW_MIN_Y as f32;
                
                // Update zoom
                game.zoom = new_zoom;
                
                // Adjust camera offset so same world position stays at screen center
                let new_effective_size = BLOCK_PIXEL_SIZE as f32 * new_zoom;
                game.camera_offset_x = screen_center_x - (world_x_at_center - VIEW_MIN_X as f32) * new_effective_size;
                game.camera_offset_y = screen_center_y - (world_y_at_center - VIEW_MIN_Y as f32) * new_effective_size;
            }
        }

        game.render();

        next_frame().await;
    }
}
