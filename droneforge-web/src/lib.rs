use droneforge_core::World;
use macroquad::prelude::*;

pub struct GameState {
    world: World,
}

impl GameState {
    pub fn new() -> Self {
        Self { world: World::new() }
    }

    fn fixed_update(&mut self) {
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
        draw_text(
            &format!("Hello"),
            20.0,
            80.0,
            24.0,
            WHITE,
        );
    }
}

pub async fn run() {
    let mut game = GameState::new();
    let mut accumulator = 0.0_f32;
    let fixed_step = 1.0_f32 / 60.0_f32; // 60 Hz simulation

    loop {
        // Consume real elapsed time in fixed-size simulation steps.
        accumulator += get_frame_time();
        while accumulator >= fixed_step {
            game.fixed_update();
            accumulator -= fixed_step;
        }

        game.render();

        next_frame().await;
    }
}

