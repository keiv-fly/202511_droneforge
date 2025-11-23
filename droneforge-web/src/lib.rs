use droneforge_core::World;
use macroquad::prelude::*;

pub struct GameState {
    world: World,
}

impl GameState {
    pub fn new() -> Self {
        Self { world: World::new() }
    }

    fn update(&mut self) {
        self.world.step();
    }

    fn render(&self) {
        clear_background(BLACK);
        draw_text(
            &format!("tick: {}", self.world.tick),
            20.0,
            40.0,
            24.0,
            WHITE,
        );
    }
}

pub async fn run() {
    let mut game = GameState::new();

    loop {
        game.update();
        game.render();

        next_frame().await;
    }
}

#[cfg(target_arch = "wasm32")]
#[macroquad::main("Droneforge Web MVP")]
async fn wasm_entry() {
    run().await;
}

