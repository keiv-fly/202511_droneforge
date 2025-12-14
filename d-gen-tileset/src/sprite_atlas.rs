use image::{GenericImage, Rgba, RgbaImage};
use roxmltree::Document;
use std::error::Error;
use std::fs;
use std::path::Path;

const DIRECTION_ROTATIONS: [f32; 4] = [-90.0, 0.0, 90.0, 180.0];
pub const SPRITE_SIZE: u32 = 256;
const SPRITE_PADDING: u32 = 2;
const SAMPLE_EPSILON: f32 = 0.25;
const AA_SAMPLES_PER_AXIS: u32 = 4;
const AA_SAMPLE_COUNT: u32 = AA_SAMPLES_PER_AXIS * AA_SAMPLES_PER_AXIS;

#[derive(Clone, Copy)]
struct SvgViewBox {
    min_x: f32,
    min_y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Copy)]
enum LineCap {
    Round,
    Butt,
}

#[derive(Clone, Copy)]
struct CircleSpec {
    cx: f32,
    cy: f32,
    r: f32,
    fill: Option<Rgba<u8>>,
    stroke: Option<Rgba<u8>>,
    stroke_width: f32,
}

#[derive(Clone, Copy)]
struct LineSpec {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    stroke: Option<Rgba<u8>>,
    stroke_width: f32,
    linecap: LineCap,
}

struct DroneSpec {
    viewbox: SvgViewBox,
    circle: CircleSpec,
    line: LineSpec,
}

pub fn build_drone_sprite_atlas(root: &Path) -> Result<RgbaImage, Box<dyn Error>> {
    let svg_dir = root.join("assets-for-gen");
    let svg_files = ["drone3.svg", "drone4.svg", "drone5.svg", "drone6.svg"];
    let mut specs = Vec::new();

    for name in svg_files {
        let path = svg_dir.join(name);
        specs.push(parse_drone_svg(&path)?);
    }

    Ok(render_atlas(&specs, SPRITE_SIZE))
}

fn parse_drone_svg(path: &Path) -> Result<DroneSpec, Box<dyn Error>> {
    let xml = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let doc = Document::parse(&xml)?;
    let svg_node = doc
        .descendants()
        .find(|node| node.has_tag_name("svg"))
        .ok_or_else(|| format!("no <svg> element found in {}", path.display()))?;

    let viewbox_attr = svg_node
        .attribute("viewBox")
        .ok_or_else(|| format!("missing viewBox on {}", path.display()))?;
    let viewbox = parse_viewbox(viewbox_attr)?;

    let circle_node = doc
        .descendants()
        .find(|node| node.has_tag_name("circle"))
        .ok_or_else(|| format!("no <circle> found in {}", path.display()))?;
    let circle = parse_circle(circle_node)?;

    let line_node = doc
        .descendants()
        .find(|node| node.has_tag_name("line"))
        .ok_or_else(|| format!("no <line> found in {}", path.display()))?;
    let line = parse_line(line_node)?;

    Ok(DroneSpec {
        viewbox,
        circle,
        line,
    })
}

fn parse_viewbox(raw: &str) -> Result<SvgViewBox, Box<dyn Error>> {
    let parts: Vec<f32> = raw
        .split_whitespace()
        .map(|part| part.parse::<f32>())
        .collect::<Result<Vec<_>, _>>()?;

    if parts.len() != 4 {
        return Err(format!("expected 4 numbers in viewBox, got {}", parts.len()).into());
    }

    Ok(SvgViewBox {
        min_x: parts[0],
        min_y: parts[1],
        width: parts[2],
        height: parts[3],
    })
}

fn parse_circle(node: roxmltree::Node) -> Result<CircleSpec, Box<dyn Error>> {
    Ok(CircleSpec {
        cx: parse_number(node.attribute("cx"), "circle cx")?,
        cy: parse_number(node.attribute("cy"), "circle cy")?,
        r: parse_number(node.attribute("r"), "circle r")?,
        fill: node.attribute("fill").and_then(parse_color),
        stroke: node.attribute("stroke").and_then(parse_color),
        stroke_width: parse_number(node.attribute("stroke-width"), "circle stroke-width")?,
    })
}

fn parse_line(node: roxmltree::Node) -> Result<LineSpec, Box<dyn Error>> {
    let linecap = match node.attribute("stroke-linecap").unwrap_or("butt") {
        "round" => LineCap::Round,
        _ => LineCap::Butt,
    };

    Ok(LineSpec {
        x1: parse_number(node.attribute("x1"), "line x1")?,
        y1: parse_number(node.attribute("y1"), "line y1")?,
        x2: parse_number(node.attribute("x2"), "line x2")?,
        y2: parse_number(node.attribute("y2"), "line y2")?,
        stroke: node.attribute("stroke").and_then(parse_color),
        stroke_width: parse_number(node.attribute("stroke-width"), "line stroke-width")?,
        linecap,
    })
}

fn parse_number(raw: Option<&str>, label: &str) -> Result<f32, Box<dyn Error>> {
    let value = raw.ok_or_else(|| format!("missing {label} attribute"))?;
    Ok(value.parse::<f32>()?)
}

fn parse_color(raw: &str) -> Option<Rgba<u8>> {
    let value = raw.trim().to_lowercase();
    match value.as_str() {
        "none" => None,
        "black" => Some(Rgba([0, 0, 0, 255])),
        "white" => Some(Rgba([255, 255, 255, 255])),
        _ if value.starts_with('#') => parse_hex_color(&value),
        _ => None,
    }
}

fn parse_hex_color(raw: &str) -> Option<Rgba<u8>> {
    let digits = raw.trim_start_matches('#');
    let (r, g, b) = match digits.len() {
        3 => {
            let r = u8::from_str_radix(&digits[0..1], 16).ok()?;
            let g = u8::from_str_radix(&digits[1..2], 16).ok()?;
            let b = u8::from_str_radix(&digits[2..3], 16).ok()?;
            (r * 17, g * 17, b * 17)
        }
        6 => {
            let r = u8::from_str_radix(&digits[0..2], 16).ok()?;
            let g = u8::from_str_radix(&digits[2..4], 16).ok()?;
            let b = u8::from_str_radix(&digits[4..6], 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };

    Some(Rgba([r, g, b, 255]))
}

fn sprite_to_padded_cell_with_extrusion(
    sprite: &RgbaImage,
    sprite_size: u32,
    pad: u32,
) -> RgbaImage {
    let stride = sprite_size + pad * 2;
    let mut cell = RgbaImage::from_pixel(stride, stride, Rgba([0, 0, 0, 0]));

    // Copy sprite into the centered area.
    for y in 0..sprite_size {
        for x in 0..sprite_size {
            cell.put_pixel(pad + x, pad + y, *sprite.get_pixel(x, y));
        }
    }

    // Extrude top/bottom edges outward into the padding.
    for x in 0..sprite_size {
        let top = *cell.get_pixel(pad + x, pad);
        let bottom = *cell.get_pixel(pad + x, pad + sprite_size - 1);
        for p in 0..pad {
            cell.put_pixel(pad + x, p, top);
            cell.put_pixel(pad + x, pad + sprite_size + p, bottom);
        }
    }

    // Extrude left/right edges outward into the padding.
    for y in 0..sprite_size {
        let left = *cell.get_pixel(pad, pad + y);
        let right = *cell.get_pixel(pad + sprite_size - 1, pad + y);
        for p in 0..pad {
            cell.put_pixel(p, pad + y, left);
            cell.put_pixel(pad + sprite_size + p, pad + y, right);
        }
    }

    // Fill corners using nearest corner pixels.
    let tl = *cell.get_pixel(pad, pad);
    let tr = *cell.get_pixel(pad + sprite_size - 1, pad);
    let bl = *cell.get_pixel(pad, pad + sprite_size - 1);
    let br = *cell.get_pixel(pad + sprite_size - 1, pad + sprite_size - 1);
    for y in 0..pad {
        for x in 0..pad {
            cell.put_pixel(x, y, tl);
            cell.put_pixel(pad + sprite_size + x, y, tr);
            cell.put_pixel(x, pad + sprite_size + y, bl);
            cell.put_pixel(pad + sprite_size + x, pad + sprite_size + y, br);
        }
    }

    cell
}

fn render_atlas(specs: &[DroneSpec], target_size: u32) -> RgbaImage {
    let columns = DIRECTION_ROTATIONS.len() as u32;
    let rows = specs.len() as u32;
    let stride = target_size + SPRITE_PADDING * 2;
    let mut atlas = RgbaImage::from_pixel(columns * stride, rows * stride, Rgba([0, 0, 0, 0]));

    for (row, spec) in specs.iter().enumerate() {
        for (col, rotation) in DIRECTION_ROTATIONS.iter().copied().enumerate() {
            let sprite = render_drone(spec, rotation, target_size);
            let sprite_cell =
                sprite_to_padded_cell_with_extrusion(&sprite, target_size, SPRITE_PADDING);
            let offset_x = (col as u32) * stride;
            let offset_y = (row as u32) * stride;

            atlas
                .copy_from(&sprite_cell, offset_x, offset_y)
                .expect("sprite cell should always fit inside the atlas");
        }
    }

    atlas
}

fn render_drone(spec: &DroneSpec, rotation_deg: f32, target_size: u32) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(target_size, target_size, Rgba([0, 0, 0, 0]));
    let scale_x = target_size as f32 / spec.viewbox.width;
    let scale_y = target_size as f32 / spec.viewbox.height;
    let scale = (scale_x + scale_y) * 0.5;
    let center_x = spec.viewbox.min_x + spec.viewbox.width * 0.5;
    let center_y = spec.viewbox.min_y + spec.viewbox.height * 0.5;
    let rotation_rad = rotation_deg.to_radians();
    let samples_per_axis = AA_SAMPLES_PER_AXIS as f32;
    let inv_sample_count = 1.0 / AA_SAMPLE_COUNT as f32;

    for y in 0..target_size {
        for x in 0..target_size {
            let mut sum = [0.0f32; 4];
            let mut has_coverage = false;

            for sy in 0..AA_SAMPLES_PER_AXIS {
                for sx in 0..AA_SAMPLES_PER_AXIS {
                    let sample_x = x as f32 + (sx as f32 + 0.5) / samples_per_axis;
                    let sample_y = y as f32 + (sy as f32 + 0.5) / samples_per_axis;
                    let svg_x = sample_x / scale + spec.viewbox.min_x;
                    let svg_y = sample_y / scale + spec.viewbox.min_y;
                    let (rot_x, rot_y) =
                        rotate_point(svg_x, svg_y, center_x, center_y, -rotation_rad);

                    if let Some(color) = sample_color(spec, rot_x, rot_y) {
                        has_coverage = true;
                        sum[0] += color.0[0] as f32;
                        sum[1] += color.0[1] as f32;
                        sum[2] += color.0[2] as f32;
                        sum[3] += color.0[3] as f32;
                    }
                }
            }

            if has_coverage {
                let pixel = Rgba([
                    (sum[0] * inv_sample_count).round() as u8,
                    (sum[1] * inv_sample_count).round() as u8,
                    (sum[2] * inv_sample_count).round() as u8,
                    (sum[3] * inv_sample_count).round() as u8,
                ]);

                image.put_pixel(x, y, pixel);
            }
        }
    }

    image
}

fn rotate_point(x: f32, y: f32, cx: f32, cy: f32, angle: f32) -> (f32, f32) {
    let dx = x - cx;
    let dy = y - cy;
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    let rotated_x = dx * cos_a - dy * sin_a + cx;
    let rotated_y = dx * sin_a + dy * cos_a + cy;
    (rotated_x, rotated_y)
}

fn sample_color(spec: &DroneSpec, svg_x: f32, svg_y: f32) -> Option<Rgba<u8>> {
    let mut color: Option<Rgba<u8>> = None;

    let dx = svg_x - spec.circle.cx;
    let dy = svg_y - spec.circle.cy;
    let dist = (dx * dx + dy * dy).sqrt();

    if let Some(fill) = spec.circle.fill {
        if dist <= spec.circle.r + SAMPLE_EPSILON {
            color = Some(fill);
        }
    }

    if let Some(stroke) = spec.circle.stroke {
        let half_width = spec.circle.stroke_width * 0.5 + SAMPLE_EPSILON;
        if dist >= spec.circle.r - half_width && dist <= spec.circle.r + half_width {
            color = Some(stroke);
        }
    }

    if let Some(line_color) = spec.line.stroke {
        if point_on_line(&spec.line, svg_x, svg_y) {
            color = Some(line_color);
        }
    }

    color
}

fn point_on_line(line: &LineSpec, x: f32, y: f32) -> bool {
    if line.stroke.is_none() || line.stroke_width <= 0.0 {
        return false;
    }

    let dx = line.x2 - line.x1;
    let dy = line.y2 - line.y1;
    let len_sq = dx * dx + dy * dy;
    let radius = line.stroke_width * 0.5 + SAMPLE_EPSILON;

    if len_sq == 0.0 {
        let dist_sq = (x - line.x1).powi(2) + (y - line.y1).powi(2);
        return dist_sq <= radius * radius;
    }

    let t = ((x - line.x1) * dx + (y - line.y1) * dy) / len_sq;
    let clamped_t = t.clamp(0.0, 1.0);
    let proj_x = line.x1 + dx * clamped_t;
    let proj_y = line.y1 + dy * clamped_t;
    let dist_sq = (x - proj_x).powi(2) + (y - proj_y).powi(2);

    match line.linecap {
        LineCap::Butt => (0.0..=1.0).contains(&t) && dist_sq <= radius * radius,
        LineCap::Round => dist_sq <= radius * radius,
    }
}
