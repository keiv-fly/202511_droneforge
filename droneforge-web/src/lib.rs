use droneforge_core::worldgen::DeterministicMap;
use droneforge_core::{
    AIR, BEDROCK, BlockId, CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, ChunkPosition, LocalBlockCoord,
    World,
};
use macroquad::prelude::*;

const VIEW_MIN_X: i32 = -100;
const VIEW_MAX_X: i32 = 100;
const VIEW_MIN_Y: i32 = -60;
const VIEW_MAX_Y: i32 = 60;
const VIEW_Z: i32 = 0;

const BLOCK_PIXEL_SIZE: u16 = 4;

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
}

impl GameState {
    pub fn new() -> Self {
        let mut world = World::new();
        let palette = BlockPalette;
        let generator = DeterministicMap::new(42);
        let chunk_z = div_floor(VIEW_Z, CHUNK_HEIGHT as i32);

        let mut chunk_textures = Vec::new();

        for chunk_y in chunk_range(VIEW_MIN_Y, VIEW_MAX_Y, CHUNK_DEPTH as i32) {
            for chunk_x in chunk_range(VIEW_MIN_X, VIEW_MAX_X, CHUNK_WIDTH as i32) {
                let position = ChunkPosition::new(chunk_x, chunk_y, chunk_z);
                world.register_generated_chunk(position, &generator);

                if let Some(chunk) = world.chunk(&position) {
                    let texture = build_chunk_texture(chunk, VIEW_Z, &palette);
                    chunk_textures.push(ChunkTexture { position, texture });
                }
            }
        }

        Self {
            world,
            chunk_textures,
        }
    }

    fn fixed_update(&mut self) {
        self.world.step();
    }

    fn render(&self) {
        clear_background(BLACK);

        let chunk_pixel_size = vec2(
            CHUNK_WIDTH as f32 * BLOCK_PIXEL_SIZE as f32,
            CHUNK_DEPTH as f32 * BLOCK_PIXEL_SIZE as f32,
        );

        for chunk_texture in &self.chunk_textures {
            let world_origin_x = chunk_texture.position.x * CHUNK_WIDTH as i32;
            let world_origin_y = chunk_texture.position.y * CHUNK_DEPTH as i32;

            let screen_x = (world_origin_x - VIEW_MIN_X) as f32 * BLOCK_PIXEL_SIZE as f32;
            let screen_y = (world_origin_y - VIEW_MIN_Y) as f32 * BLOCK_PIXEL_SIZE as f32;

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
    let mut accumulator = 0.0_f32;
    let fixed_step = 1.0_f32 / 60.0_f32; // 60 Hz simulation

    loop {
        // Consume real elapsed time in fixed-size simulation steps.
        accumulator += get_frame_time();
        while accumulator >= fixed_step {
            game.fixed_update();
            accumulator -= fixed_step;
        }

        game.render();

        next_frame().await;
    }
}
