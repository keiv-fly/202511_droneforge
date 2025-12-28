use macroquad::prelude::{draw_poly, Color, Vec2};
use std::f32::consts::FRAC_PI_4;

pub const CORE_COLOR: Color = Color::from_rgba(0, 53, 146, 255);

/// Draw a diamond at a screen-space center using vector primitives to stay crisp when zoomed.
pub fn draw_core_at_screen(center: Vec2, tile_size: f32) {
    let half = tile_size * 0.5;
    let margin = tile_size * 0.2;
    let radius = (half - margin).max(1.0);
    draw_poly(center.x, center.y, 4, radius, FRAC_PI_4, CORE_COLOR);
}

