use crate::block::{AIR, BlockId};
use crate::coordinates::WorldCoord;

fn is_solid(block: Option<BlockId>) -> bool {
    block.map_or(false, |b| b != AIR)
}

/// Returns the first solid block encountered along the line from `start` to `end`,
/// using a supercover Bresenham traversal across the XY plane. The start tile is
/// ignored, allowing a drone to move out of its current tile even if occupied.
pub fn first_solid_supercover<F>(
    mut block_at_world: F,
    start: WorldCoord,
    end: WorldCoord,
) -> Option<WorldCoord>
where
    F: FnMut(WorldCoord) -> Option<BlockId>,
{
    debug_assert_eq!(start.z, end.z, "line collision assumes a shared z level");

    let mut x0 = start.x;
    let mut y0 = start.y;
    let x1 = end.x;
    let y1 = end.y;
    let z = start.z;

    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();

    let sx = if x0 < x1 {
        1
    } else if x0 > x1 {
        -1
    } else {
        0
    };
    let sy = if y0 < y1 {
        1
    } else if y0 > y1 {
        -1
    } else {
        0
    };

    let mut err = dx - dy;
    let mut first_point = true;

    loop {
        let current = WorldCoord::new(x0, y0, z);
        if !first_point && is_solid(block_at_world(current)) {
            return Some(current);
        }
        first_point = false;

        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = err.saturating_mul(2);

        let mut step_x = false;
        let mut step_y = false;

        if e2 > -dy {
            err -= dy;
            x0 += sx;
            step_x = true;
        }

        if e2 < dx {
            err += dx;
            y0 += sy;
            step_y = true;
        }

        // When the line is perfectly diagonal (error == 0), we step both axes.
        // Check both orthogonal neighbours to ensure the path does not clip a corner.
        if step_x && step_y {
            let neighbour_x = WorldCoord::new(x0 - sx, y0, z);
            if neighbour_x != start && is_solid(block_at_world(neighbour_x)) {
                return Some(neighbour_x);
            }

            let neighbour_y = WorldCoord::new(x0, y0 - sy, z);
            if neighbour_y != start && is_solid(block_at_world(neighbour_y)) {
                return Some(neighbour_y);
            }
        }
    }

    None
}
