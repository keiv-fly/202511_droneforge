use droneforge_core::DronePose;
use macroquad::prelude::*;

#[derive(Debug, Clone)]
pub struct DroneDrawConfig {
    pub radius_tiles: f32,        // inner radius before stroke
    pub stroke_ratio: f32,        // stroke width as a fraction of radius
    pub line_start_ratio: f32,    // where the line begins, fraction of radius
    pub line_length_ratio: f32,   // line length as a fraction of radius
    pub line_thickness_ratio: f32, // line thickness as a fraction of radius
    pub stroke_color: Color,
    pub fill_color: Color,
}

impl Default for DroneDrawConfig {
    fn default() -> Self {
        Self {
            radius_tiles: 0.4,
            stroke_ratio: 0.4,       // matches drone7.svg: 32px stroke over 80px radius
            line_start_ratio: 1.0 / 3.0,
            line_length_ratio: 2.0 / 3.0,
            line_thickness_ratio: 0.4,
            stroke_color: BLACK,
            fill_color: WHITE,
        }
    }
}

pub fn is_visible_at_view(drone: &DronePose, view_z: i32) -> bool {
    let z_level = drone.position[2].floor() as i32;
    z_level == view_z || z_level == view_z.saturating_add(1)
}

pub fn drone_world_center(drone: &DronePose) -> Vec2 {
    vec2(drone.position[0] + 0.5, drone.position[1] + 0.5)
}

fn normalized_heading(drone: &DronePose) -> Vec2 {
    let mut heading = vec2(drone.heading[0], drone.heading[1]);
    if heading.length_squared() <= f32::EPSILON {
        heading = vec2(1.0, 0.0);
    }
    heading.normalize_or_zero()
}

pub fn draw_drone(
    drone: &DronePose,
    center_screen: Vec2,
    effective_block_size: f32,
    config: &DroneDrawConfig,
) {
    let radius_px = config.radius_tiles * effective_block_size;
    let stroke_px = (radius_px * config.stroke_ratio).max(1.0);
    let outer_radius_px = radius_px + stroke_px * 0.5;
    let inner_radius_px = (radius_px - stroke_px * 0.5).max(0.0);
    let heading = normalized_heading(drone);

    // Start inside the circle and extend to the edge, matching drone7 proportions.
    let line_start = center_screen + heading * (radius_px * config.line_start_ratio);
    let line_end = line_start + heading * (radius_px * config.line_length_ratio);
    let line_thickness_px = (radius_px * config.line_thickness_ratio).max(1.0);

    // Stroke + fill to emulate stroked circle with white interior.
    draw_circle(
        center_screen.x,
        center_screen.y,
        outer_radius_px,
        config.stroke_color,
    );
    if inner_radius_px > 0.0 {
        draw_circle(
            center_screen.x,
            center_screen.y,
            inner_radius_px,
            config.fill_color,
        );
    }
    draw_line(
        line_start.x,
        line_start.y,
        line_end.x,
        line_end.y,
        line_thickness_px,
        config.stroke_color,
    );
    draw_circle(
        line_start.x,
        line_start.y,
        line_thickness_px * 0.5,
        config.stroke_color,
    );
    draw_circle(
        line_end.x,
        line_end.y,
        line_thickness_px * 0.5,
        config.stroke_color,
    );
}

