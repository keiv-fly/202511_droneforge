#[derive(Debug, Clone, PartialEq)]
pub struct DronePose {
    pub position: [f32; 3], // world-space tile coordinates, fractional allowed
    pub heading: [f32; 2],  // normalized direction; defaults handled by consumers
    pub name: String,
    pub health: i32,
    pub max_health: i32,
}

impl DronePose {
    pub fn new(
        position: [f32; 3],
        heading: [f32; 2],
        name: impl Into<String>,
        health: i32,
        max_health: i32,
    ) -> Self {
        let bounded_max = max_health.max(1);
        let clamped_health = health.clamp(0, bounded_max);
        Self {
            position,
            heading,
            name: name.into(),
            health: clamped_health,
            max_health: bounded_max,
        }
    }
}
