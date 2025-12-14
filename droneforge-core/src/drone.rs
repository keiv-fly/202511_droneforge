#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DronePose {
    pub position: [f32; 3], // world-space tile coordinates, fractional allowed
    pub heading: [f32; 2],  // normalized direction; defaults handled by consumers
}

impl DronePose {
    pub fn new(position: [f32; 3], heading: [f32; 2]) -> Self {
        Self { position, heading }
    }
}

