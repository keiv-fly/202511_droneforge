use image::{Rgba, RgbaImage};

use crate::layout;
use droneforge_core::{AIR, BEDROCK, BlockId, DIRT, IRON, STONE};

#[derive(Clone, Copy)]
struct Color {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

fn color_from_rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
    }
}

fn color_to_rgba(color: Color) -> Rgba<u8> {
    let to_u8 = |channel: f32| -> u8 { (channel.clamp(0.0, 1.0) * 255.0).round() as u8 };
    Rgba([
        to_u8(color.r),
        to_u8(color.g),
        to_u8(color.b),
        to_u8(color.a),
    ])
}

fn palette_color(block: BlockId) -> Color {
    match block {
        AIR => color_from_rgba(0, 0, 0, 0),
        DIRT => color_from_rgba(143, 99, 63, 255),
        STONE => color_from_rgba(120, 120, 120, 255),
        IRON => color_from_rgba(194, 133, 74, 255),
        BEDROCK => color_from_rgba(45, 45, 45, 255),
        _ => color_from_rgba(255, 0, 255, 255),
    }
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
    let target_total = (size / 4).max(1);
    let outline = wall_outline_thickness(size);
    target_total.saturating_sub(outline).max(1)
}

fn fill_block(image: &mut RgbaImage, tile_x: u32, tile_y: u32, color: Color) {
    let pixel_size = layout::BLOCK_PIXEL_SIZE;
    let start_x = tile_x * pixel_size;
    let start_y = tile_y * pixel_size;
    let rgba = color_to_rgba(color);

    for dy in 0..pixel_size {
        for dx in 0..pixel_size {
            image.put_pixel(start_x + dx, start_y + dy, rgba);
        }
    }
}

fn fill_rect(
    image: &mut RgbaImage,
    start_x: u32,
    start_y: u32,
    width: u32,
    height: u32,
    color: Color,
) {
    let rgba = color_to_rgba(color);

    for dy in 0..height {
        for dx in 0..width {
            image.put_pixel(start_x + dx, start_y + dy, rgba);
        }
    }
}

fn draw_wall_overlay(
    image: &mut RgbaImage,
    tile_x: u32,
    tile_y: u32,
    base_color: Color,
    edge_color: Color,
    mask: u8,
) {
    fill_block(image, tile_x, tile_y, base_color);

    let size = layout::BLOCK_PIXEL_SIZE;
    let start_x = tile_x * size;
    let start_y = tile_y * size;
    let outline_thickness = wall_outline_thickness(size);
    let rim_thickness = wall_rim_thickness(size);

    let north_y = start_y + outline_thickness;
    let south_y = start_y + size.saturating_sub(outline_thickness + rim_thickness);
    let west_x = start_x + outline_thickness;
    let east_x = start_x + size.saturating_sub(outline_thickness + rim_thickness);

    if mask & layout::MASK_NORTH != 0 {
        fill_rect(image, start_x, north_y, size, rim_thickness, edge_color);
    }

    if mask & layout::MASK_EAST != 0 {
        fill_rect(image, east_x, start_y, rim_thickness, size, edge_color);
    }

    if mask & layout::MASK_SOUTH != 0 {
        fill_rect(image, start_x, south_y, size, rim_thickness, edge_color);
    }

    if mask & layout::MASK_WEST != 0 {
        fill_rect(image, west_x, start_y, rim_thickness, size, edge_color);
    }
}

fn draw_wall_outline(image: &mut RgbaImage, tile_x: u32, tile_y: u32, mask: u8) {
    if mask == 0 {
        return;
    }

    let size = layout::BLOCK_PIXEL_SIZE;
    let outline_thickness = wall_outline_thickness(size);
    let start_x = tile_x * size;
    let start_y = tile_y * size;
    let black = color_from_rgba(0, 0, 0, 255);

    if mask & layout::MASK_NORTH != 0 {
        fill_rect(image, start_x, start_y, size, outline_thickness, black);
    }

    if mask & layout::MASK_EAST != 0 {
        fill_rect(
            image,
            start_x + size.saturating_sub(outline_thickness),
            start_y,
            outline_thickness,
            size,
            black,
        );
    }

    if mask & layout::MASK_SOUTH != 0 {
        fill_rect(
            image,
            start_x,
            start_y + size.saturating_sub(outline_thickness),
            size,
            outline_thickness,
            black,
        );
    }

    if mask & layout::MASK_WEST != 0 {
        fill_rect(image, start_x, start_y, outline_thickness, size, black);
    }

    if mask & layout::MASK_NORTH != 0 && mask & layout::MASK_WEST != 0 {
        fill_rect(
            image,
            start_x,
            start_y,
            outline_thickness,
            outline_thickness,
            black,
        );
    }

    if mask & layout::MASK_NORTH != 0 && mask & layout::MASK_EAST != 0 {
        fill_rect(
            image,
            start_x + size.saturating_sub(outline_thickness),
            start_y,
            outline_thickness,
            outline_thickness,
            black,
        );
    }

    if mask & layout::MASK_SOUTH != 0 && mask & layout::MASK_WEST != 0 {
        fill_rect(
            image,
            start_x,
            start_y + size.saturating_sub(outline_thickness),
            outline_thickness,
            outline_thickness,
            black,
        );
    }

    if mask & layout::MASK_SOUTH != 0 && mask & layout::MASK_EAST != 0 {
        fill_rect(
            image,
            start_x + size.saturating_sub(outline_thickness),
            start_y + size.saturating_sub(outline_thickness),
            outline_thickness,
            outline_thickness,
            black,
        );
    }
}

pub fn build_tileset_image() -> RgbaImage {
    let (width_px, height_px) = layout::atlas_pixel_size();
    let mut atlas = RgbaImage::from_pixel(width_px, height_px, Rgba([0, 0, 0, 0]));
    let wall_masks = 0u8..(layout::WALL_MASK_VARIANTS as u8);

    for &block in layout::SOLID_BLOCKS.iter() {
        let base_color = palette_color(block);
        let pos = layout::floor_tile_position(block)
            .expect("floor tile position should exist for solid block");
        fill_block(&mut atlas, pos.tile_x, pos.tile_y, base_color);
    }

    for &block in layout::SOLID_BLOCKS.iter() {
        let source_color = palette_color(block);
        let base_color = wall_base_tint(source_color);
        let edge_color = wall_edge_tint(source_color);

        for mask in wall_masks.clone() {
            let pos = layout::wall_tile_position(block, mask)
                .expect("wall tile position should exist for solid block and mask");
            draw_wall_overlay(
                &mut atlas, pos.tile_x, pos.tile_y, base_color, edge_color, mask,
            );
            draw_wall_outline(&mut atlas, pos.tile_x, pos.tile_y, mask);
        }
    }

    atlas
}
