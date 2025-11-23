use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub tick: u64,
}

impl World {
    pub fn new() -> Self {
        Self { tick: 0 }
    }

    pub fn step(&mut self) {
        self.tick += 1;
    }
}
