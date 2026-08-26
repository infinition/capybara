use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MiniGameType {
    BerryCatch,
    ParadiseWheel,
}

#[derive(Debug, Clone)]
pub struct BerryItem {
    pub x: f32, // 0.0 to 1.0
    pub y: f32, // 0.0 to 1.0
    pub speed: f32,
}

#[derive(Debug, Clone)]
pub struct BerryCatchState {
    pub basket_x: f32, // 0.0 to 1.0
    pub berries: Vec<BerryItem>,
    pub score: u32,
    pub time_remaining: f32,
    pub is_finished: bool,
    pub spawn_timer: f32,
}

impl BerryCatchState {
    pub fn new() -> Self {
        Self {
            basket_x: 0.5,
            berries: Vec::new(),
            score: 0,
            time_remaining: 20.0,
            is_finished: false,
            spawn_timer: 0.0,
        }
    }

    pub fn move_left(&mut self, dt: f32) {
        self.basket_x = (self.basket_x - 0.7 * dt).max(0.1);
    }

    pub fn move_right(&mut self, dt: f32) {
        self.basket_x = (self.basket_x + 0.7 * dt).min(0.9);
    }

    pub fn update(&mut self, dt: f32) -> Option<bool> {
        if self.is_finished {
            return None;
        }

        self.time_remaining -= dt;
        if self.time_remaining <= 0.0 {
            self.is_finished = true;
            return Some(self.score >= 5);
        }

        self.spawn_timer += dt;
        if self.spawn_timer >= 0.8 {
            self.spawn_timer = 0.0;
            let mut rng = rand::thread_rng();
            let x: f32 = rng.gen_range(0.1..0.9);
            let speed: f32 = rng.gen_range(0.3..0.6);
            self.berries.push(BerryItem { x, y: 0.0, speed });
        }

        let mut caught = 0;
        self.berries.retain_mut(|b| {
            b.y += b.speed * dt;
            if b.y >= 0.85 && b.y <= 0.95 && (b.x - self.basket_x).abs() < 0.12 {
                caught += 1;
                false
            } else {
                b.y <= 1.0
            }
        });

        self.score += caught;
        None
    }
}

#[derive(Debug, Clone)]
pub struct ParadiseWheelState {
    pub current_angle: f32, // 0.0 to 360.0
    pub target_min: f32,
    pub target_max: f32,
    pub is_spinning: bool,
    pub is_finished: bool,
    pub is_won: bool,
}

impl ParadiseWheelState {
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        let target_start: f32 = rng.gen_range(60.0..300.0);
        Self {
            current_angle: 0.0,
            target_min: target_start,
            target_max: target_start + 45.0,
            is_spinning: true,
            is_finished: false,
            is_won: false,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.is_spinning {
            self.current_angle = (self.current_angle + 280.0 * dt) % 360.0;
        }
    }

    pub fn stop(&mut self) -> bool {
        if !self.is_spinning || self.is_finished {
            return self.is_won;
        }

        self.is_spinning = false;
        self.is_finished = true;
        self.is_won =
            self.current_angle >= self.target_min && self.current_angle <= self.target_max;
        self.is_won
    }
}
