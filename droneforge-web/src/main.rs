use droneforge_core::World;
use macroquad::prelude::*;

struct GameState {
    world: World,
}

impl GameState {
    fn new() -> Self {
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

#[macroquad::main("Droneforge Web MVP")]
async fn main() {
    let mut game = GameState::new();

    loop {
        game.update();
        game.render();

        next_frame().await;
    }
}
