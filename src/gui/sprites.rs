use egui::Color32;

use crate::core::{GrowthStage, Species};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpriteState {
    Idle,
    Happy,
    Eating,
    Sleeping,
    Sick,
    Walking,
}

pub struct SpriteSheet;

impl SpriteSheet {
    pub const SIZE: usize = 20;

    pub fn get_pet_pixels(
        stage: GrowthStage,
        species: Species,
        state: SpriteState,
        frame: usize,
    ) -> Vec<Vec<Color32>> {
        let mut grid = vec![vec![Color32::TRANSPARENT; Self::SIZE]; Self::SIZE];
        let bob = if frame % 2 == 0 { 0 } else { 1 };

        let primary_color = match species {
            Species::EggNormal => Color32::from_rgb(255, 235, 180),
            Species::Babytchi => Color32::from_rgb(255, 255, 255),
            Species::Marutchi => Color32::from_rgb(255, 180, 200),
            Species::Tamatchi => Color32::from_rgb(255, 215, 0),
            Species::YoungMametchi | Species::Mametchi => Color32::from_rgb(255, 230, 80),
            Species::YoungMemetchi | Species::Memetchi => Color32::from_rgb(255, 140, 200),
            Species::YoungKuchipatchi | Species::Kuchipatchi => {
                Color32::from_rgb(120, 220, 100)
            }
            Species::Floragitchi => Color32::from_rgb(255, 160, 210),
            Species::Coralgotchi => Color32::from_rgb(80, 200, 240),
            Species::Skygotchi => Color32::from_rgb(180, 160, 255),
        };

        let outline = Color32::from_rgb(30, 30, 40);
        let blush = Color32::from_rgb(255, 100, 130);
        let eye = Color32::from_rgb(20, 20, 20);

        if stage == GrowthStage::Egg {
            // Draw egg with spots
            for y in 4..16 {
                for x in 6..14 {
                    let dx = (x as f32 - 9.5).abs();
                    let dy = (y as f32 - 10.0).abs();
                    if dx * dx / 16.0 + dy * dy / 36.0 <= 1.0 {
                        grid[y][x] = primary_color;
                    }
                }
            }
            // Egg spots
            grid[7][8] = Color32::from_rgb(100, 200, 255);
            grid[11][11] = Color32::from_rgb(255, 120, 180);
            grid[12][7] = Color32::from_rgb(120, 230, 120);

            // Egg wobble on frame
            if frame % 2 == 1 {
                grid[4][9] = outline;
            }
            return grid;
        }

        let base_y = 5 + bob;

        // Draw body shape
        let radius_x = match stage {
            GrowthStage::Baby => 4,
            GrowthStage::Child => 5,
            GrowthStage::Teen => 6,
            _ => 7,
        };
        let radius_y = match stage {
            GrowthStage::Baby => 4,
            GrowthStage::Child => 5,
            GrowthStage::Teen => 6,
            _ => 7,
        };

        let center_x = 10;
        let center_y = base_y + radius_y;

        for y in (center_y - radius_y)..=(center_y + radius_y) {
            for x in (center_x - radius_x)..=(center_x + radius_x) {
                if x < Self::SIZE && y < Self::SIZE {
                    let dx = (x as i32 - center_x as i32).abs() as f32;
                    let dy = (y as i32 - center_y as i32).abs() as f32;
                    if (dx * dx) / (radius_x as f32 * radius_x as f32)
                        + (dy * dy) / (radius_y as f32 * radius_y as f32)
                        <= 1.0
                    {
                        grid[y][x] = primary_color;
                    }
                }
            }
        }

        // Add Ears / Hats / Features based on species
        match species {
            Species::Mametchi | Species::YoungMametchi => {
                // Tall black ears
                for ey in (base_y.saturating_sub(4))..base_y {
                    if ey < Self::SIZE {
                        grid[ey][center_x - 4] = outline;
                        grid[ey][center_x - 3] = outline;
                        grid[ey][center_x + 3] = outline;
                        grid[ey][center_x + 4] = outline;
                    }
                }
            }
            Species::Memetchi | Species::YoungMemetchi => {
                // Swirly head curl
                let ey = base_y.saturating_sub(3);
                if ey < Self::SIZE {
                    grid[ey][center_x] = primary_color;
                    grid[ey + 1][center_x - 1] = primary_color;
                    grid[ey + 1][center_x + 1] = primary_color;
                }
            }
            Species::Kuchipatchi | Species::YoungKuchipatchi => {
                // Big duck lips
                let my = center_y + 1;
                grid[my][center_x - 2] = Color32::from_rgb(80, 180, 70);
                grid[my][center_x - 1] = Color32::from_rgb(80, 180, 70);
                grid[my][center_x] = Color32::from_rgb(80, 180, 70);
                grid[my][center_x + 1] = Color32::from_rgb(80, 180, 70);
            }
            Species::Floragitchi => {
                // Flower on head
                let ey = base_y.saturating_sub(2);
                if ey < Self::SIZE {
                    grid[ey][center_x] = Color32::from_rgb(255, 230, 50);
                    grid[ey][center_x - 2] = Color32::from_rgb(255, 100, 180);
                    grid[ey][center_x + 2] = Color32::from_rgb(255, 100, 180);
                }
            }
            Species::Coralgotchi => {
                // Crown / coral horns
                let ey = base_y.saturating_sub(2);
                if ey < Self::SIZE {
                    grid[ey][center_x - 3] = Color32::from_rgb(255, 120, 100);
                    grid[ey][center_x + 3] = Color32::from_rgb(255, 120, 100);
                }
            }
            _ => {}
        }

        // Eyes & Mouth based on state
        match state {
            SpriteState::Sleeping => {
                // Closed eyes: - -
                grid[center_y - 1][center_x - 2] = eye;
                grid[center_y - 1][center_x - 3] = eye;
                grid[center_y - 1][center_x + 2] = eye;
                grid[center_y - 1][center_x + 3] = eye;
                // Zzz in corner
                grid[2][15] = Color32::from_rgb(100, 180, 255);
                grid[3][16] = Color32::from_rgb(100, 180, 255);
                grid[4][15] = Color32::from_rgb(100, 180, 255);
            }
            SpriteState::Sick => {
                // X eyes
                grid[center_y - 2][center_x - 3] = eye;
                grid[center_y - 1][center_x - 2] = eye;
                grid[center_y - 2][center_x + 3] = eye;
                grid[center_y - 1][center_x + 2] = eye;
                // Skull in top corner
                grid[2][16] = Color32::from_rgb(180, 100, 220);
                grid[3][16] = Color32::from_rgb(180, 100, 220);
            }
            SpriteState::Happy => {
                // Sparkle curved eyes ^ ^
                grid[center_y - 2][center_x - 3] = eye;
                grid[center_y - 2][center_x - 2] = eye;
                grid[center_y - 1][center_x - 4] = eye;
                grid[center_y - 2][center_x + 3] = eye;
                grid[center_y - 2][center_x + 2] = eye;
                grid[center_y - 1][center_x + 4] = eye;

                // Big open mouth :D
                grid[center_y + 1][center_x] = outline;
                grid[center_y + 2][center_x - 1] = outline;
                grid[center_y + 2][center_x + 1] = outline;

                // Cheeks
                grid[center_y][center_x - 4] = blush;
                grid[center_y][center_x + 4] = blush;
            }
            _ => {
                // Normal eyes
                grid[center_y - 1][center_x - 2] = eye;
                grid[center_y][center_x - 2] = eye;
                grid[center_y - 1][center_x + 2] = eye;
                grid[center_y][center_x + 2] = eye;

                // Cheeks
                grid[center_y + 1][center_x - 4] = blush;
                grid[center_y + 1][center_x + 4] = blush;

                // Mouth
                grid[center_y + 1][center_x] = outline;
            }
        }

        // Feet
        let foot_y = (center_y + radius_y).min(Self::SIZE - 2);
        grid[foot_y][center_x - 3] = outline;
        grid[foot_y + 1][center_x - 3] = outline;
        grid[foot_y][center_x + 3] = outline;
        grid[foot_y + 1][center_x + 3] = outline;

        grid
    }

    pub fn get_poop_pixels() -> Vec<Vec<Color32>> {
        let mut grid = vec![vec![Color32::TRANSPARENT; 8]; 8];
        let brown = Color32::from_rgb(180, 100, 40);
        let dark_brown = Color32::from_rgb(120, 60, 20);

        grid[1][4] = brown;
        grid[2][3] = brown;
        grid[2][4] = brown;
        grid[3][2] = brown;
        grid[3][3] = dark_brown;
        grid[3][4] = brown;
        grid[3][5] = brown;
        for x in 1..7 {
            grid[4][x] = brown;
            grid[5][x] = dark_brown;
            grid[6][x] = brown;
        }
        grid
    }
}
