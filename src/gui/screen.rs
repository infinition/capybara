use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Stroke};

use crate::core::garden::BiomeType;
use crate::core::minigames::{BerryCatchState, ParadiseWheelState};
use crate::core::{ParadiseIsland, Pet};
use crate::gui::sprites::{SpriteSheet, SpriteState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomLevel {
    Micro = 0,
    Normal = 1,
    Paradise = 2,
}

impl ZoomLevel {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => ZoomLevel::Micro,
            2 => ZoomLevel::Paradise,
            _ => ZoomLevel::Normal,
        }
    }

    pub fn title_key(&self) -> &'static str {
        match self {
            ZoomLevel::Micro => "zoom_micro",
            ZoomLevel::Normal => "zoom_normal",
            ZoomLevel::Paradise => "zoom_paradise",
        }
    }
}

pub struct VirtualScreen {
    pub zoom: ZoomLevel,
    pub anim_frame: usize,
    pub anim_timer: f32,
    pub dialog_message: Option<(String, f32)>, // message and remaining duration
}

impl Default for VirtualScreen {
    fn default() -> Self {
        Self {
            zoom: ZoomLevel::Normal,
            anim_frame: 0,
            anim_timer: 0.0,
            dialog_message: None,
        }
    }
}

impl VirtualScreen {
    pub fn update(&mut self, dt: f32) {
        self.anim_timer += dt;
        if self.anim_timer >= 0.4 {
            self.anim_timer = 0.0;
            self.anim_frame = (self.anim_frame + 1) % 4;
        }

        if let Some((_, duration)) = &mut self.dialog_message {
            *duration -= dt;
            if *duration <= 0.0 {
                self.dialog_message = None;
            }
        }
    }

    pub fn show_message(&mut self, msg: String) {
        self.dialog_message = Some((msg, 3.5));
    }

    pub fn render(
        &self,
        painter: &Painter,
        rect: Rect,
        pet: &Pet,
        island: &ParadiseIsland,
        berry_game: Option<&BerryCatchState>,
        wheel_game: Option<&ParadiseWheelState>,
    ) {
        // Draw LCD bezel and glass background
        painter.rect_filled(rect, 10.0, Color32::from_rgb(180, 205, 170));
        painter.rect_stroke(
            rect,
            10.0,
            Stroke::new(3.0_f32, Color32::from_rgb(100, 130, 90)),
        );

        if let Some(bg) = berry_game {
            self.render_berry_game(painter, rect, bg);
            return;
        }

        if let Some(wg) = wheel_game {
            self.render_wheel_game(painter, rect, wg);
            return;
        }

        match self.zoom {
            ZoomLevel::Micro => self.render_micro_view(painter, rect, island),
            ZoomLevel::Normal => self.render_normal_view(painter, rect, pet),
            ZoomLevel::Paradise => self.render_paradise_view(painter, rect, island, pet),
        }

        // Render message banner if present
        if let Some((msg, _)) = &self.dialog_message {
            let banner_rect = Rect::from_min_size(
                pos2(rect.min.x + 8.0, rect.max.y - 32.0),
                vec2(rect.width() - 16.0, 24.0),
            );
            painter.rect_filled(
                banner_rect,
                4.0,
                Color32::from_rgba_premultiplied(20, 30, 20, 220),
            );
            painter.text(
                banner_rect.center(),
                egui::Align2::CENTER_CENTER,
                msg,
                egui::FontId::proportional(12.0),
                Color32::from_rgb(240, 255, 230),
            );
        }
    }

    fn render_normal_view(&self, painter: &Painter, rect: Rect, pet: &Pet) {
        // Draw room floor line
        let floor_y = rect.min.y + rect.height() * 0.78;
        painter.line_segment(
            [
                pos2(rect.min.x + 10.0, floor_y),
                pos2(rect.max.x - 10.0, floor_y),
            ],
            Stroke::new(2.0_f32, Color32::from_rgb(120, 150, 110)),
        );

        // Draw Room window / sun / moon
        let window_rect = Rect::from_min_size(
            pos2(rect.min.x + 16.0, rect.min.y + 16.0),
            vec2(28.0, 28.0),
        );
        painter.rect_stroke(
            window_rect,
            2.0,
            Stroke::new(1.5_f32, Color32::from_rgb(120, 150, 110)),
        );
        if pet.is_sleeping {
            painter.circle_filled(window_rect.center(), 6.0, Color32::from_rgb(80, 110, 80));
        } else {
            painter.circle_filled(
                window_rect.center(),
                6.0,
                Color32::from_rgb(240, 200, 50),
            );
        }

        // Draw character sprite
        let sprite_state = if pet.is_sick {
            SpriteState::Sick
        } else if pet.is_sleeping {
            SpriteState::Sleeping
        } else if pet.happiness >= 3 {
            SpriteState::Happy
        } else {
            SpriteState::Idle
        };

        let pixels = SpriteSheet::get_pet_pixels(
            pet.stage,
            pet.species,
            sprite_state,
            self.anim_frame,
        );

        let pixel_size = (rect.height() * 0.45) / (SpriteSheet::SIZE as f32);
        let sprite_start = pos2(
            rect.center().x - (SpriteSheet::SIZE as f32 * pixel_size) / 2.0,
            floor_y - (SpriteSheet::SIZE as f32 * pixel_size) + 4.0,
        );

        self.draw_pixel_matrix(painter, sprite_start, &pixels, pixel_size);

        // Draw Poops
        if pet.poop_count > 0 {
            let poop_pixels = SpriteSheet::get_poop_pixels();
            let poop_pixel_size = pixel_size * 0.7;
            for i in 0..pet.poop_count {
                let poop_pos = pos2(
                    rect.max.x - 30.0 - (i as f32 * 18.0),
                    floor_y - 12.0,
                );
                self.draw_pixel_matrix(painter, poop_pos, &poop_pixels, poop_pixel_size);
            }
        }
    }

    fn render_micro_view(&self, painter: &Painter, rect: Rect, island: &ParadiseIsland) {
        painter.rect_filled(rect, 10.0, Color32::from_rgb(130, 170, 140));

        let cell_center = rect.center();
        let pulse = (self.anim_frame as f32 * 1.5).sin() * 4.0;
        let radius = rect.height() * 0.28 + pulse;

        // Big cell organism
        painter.circle_filled(
            cell_center,
            radius,
            Color32::from_rgb(160, 215, 170),
        );
        painter.circle_stroke(
            cell_center,
            radius,
            Stroke::new(2.5_f32, Color32::from_rgb(60, 100, 70)),
        );

        // Nucleus
        painter.circle_filled(
            pos2(cell_center.x - 10.0, cell_center.y - 10.0),
            12.0,
            Color32::from_rgb(90, 140, 100),
        );

        // Floating particles
        let num_particles = 6;
        for i in 0..num_particles {
            let angle = (i as f32 * 60.0 + self.anim_frame as f32 * 20.0).to_radians();
            let dist = radius + 18.0;
            let px = cell_center.x + angle.cos() * dist;
            let py = cell_center.y + angle.sin() * dist;
            painter.circle_filled(pos2(px, py), 4.0, Color32::from_rgb(220, 140, 120));
        }

        // Micro Health gauge
        let gauge_rect = Rect::from_min_size(
            pos2(rect.min.x + 16.0, rect.min.y + 16.0),
            vec2(rect.width() - 32.0, 12.0),
        );
        painter.rect_stroke(
            gauge_rect,
            4.0,
            Stroke::new(1.5_f32, Color32::from_rgb(40, 70, 40)),
        );
        let fill_w = (gauge_rect.width() - 4.0) * (island.micro_cell_health / 100.0);
        let fill_rect = Rect::from_min_size(
            pos2(gauge_rect.min.x + 2.0, gauge_rect.min.y + 2.0),
            vec2(fill_w, gauge_rect.height() - 4.0),
        );
        painter.rect_filled(fill_rect, 2.0, Color32::from_rgb(80, 180, 90));
    }

    fn render_paradise_view(
        &self,
        painter: &Painter,
        rect: Rect,
        island: &ParadiseIsland,
        pet: &Pet,
    ) {
        let (sky_col, ground_col) = match island.active_biome {
            BiomeType::Garden => (
                Color32::from_rgb(170, 210, 240),
                Color32::from_rgb(120, 190, 110),
            ),
            BiomeType::Ocean => (
                Color32::from_rgb(150, 200, 255),
                Color32::from_rgb(70, 160, 220),
            ),
            BiomeType::Sky => (
                Color32::from_rgb(200, 180, 255),
                Color32::from_rgb(230, 220, 255),
            ),
        };

        painter.rect_filled(rect, 10.0, sky_col);

        let horizon_y = rect.min.y + rect.height() * 0.55;
        let ground_rect = Rect::from_min_max(
            pos2(rect.min.x, horizon_y),
            pos2(rect.max.x, rect.max.y),
        );
        painter.rect_filled(ground_rect, 10.0, ground_col);

        // Island Tree with Fruits
        let tree_pos = pos2(rect.min.x + 40.0, horizon_y - 20.0);
        painter.circle_filled(tree_pos, 22.0, Color32::from_rgb(60, 140, 60));
        painter.rect_filled(
            Rect::from_min_size(pos2(tree_pos.x - 4.0, tree_pos.y), vec2(8.0, 24.0)),
            2.0,
            Color32::from_rgb(120, 80, 40),
        );

        // Draw Fruits on tree
        for i in 0..island.flora_growth_level.min(5) {
            let fx = tree_pos.x - 12.0 + (i as f32 * 6.0);
            let fy = tree_pos.y - 8.0 + ((i % 2) as f32 * 6.0);
            painter.circle_filled(pos2(fx, fy), 4.0, Color32::from_rgb(230, 60, 70));
        }

        // Small character exploring island
        let mini_pixels = SpriteSheet::get_pet_pixels(
            pet.stage,
            pet.species,
            SpriteState::Walking,
            self.anim_frame,
        );
        let mini_pixel_size = 2.2;
        let char_pos = pos2(rect.center().x + 20.0, horizon_y + 10.0);
        self.draw_pixel_matrix(painter, char_pos, &mini_pixels, mini_pixel_size);
    }

    fn render_berry_game(&self, painter: &Painter, rect: Rect, game: &BerryCatchState) {
        painter.rect_filled(rect, 10.0, Color32::from_rgb(160, 200, 240));

        let score_text =
            format!("Score: {}  Time: {:.0}s", game.score, game.time_remaining);
        painter.text(
            pos2(rect.center().x, rect.min.y + 16.0),
            egui::Align2::CENTER_CENTER,
            score_text,
            egui::FontId::monospace(13.0),
            Color32::from_rgb(20, 30, 50),
        );

        for b in &game.berries {
            let bx = rect.min.x + b.x * rect.width();
            let by = rect.min.y + b.y * rect.height();
            painter.circle_filled(pos2(bx, by), 6.0, Color32::from_rgb(220, 50, 70));
        }

        let basket_x = rect.min.x + game.basket_x * rect.width();
        let basket_y = rect.min.y + 0.88 * rect.height();
        let basket_rect =
            Rect::from_center_size(pos2(basket_x, basket_y), vec2(36.0, 14.0));
        painter.rect_filled(basket_rect, 4.0, Color32::from_rgb(160, 100, 50));
    }

    fn render_wheel_game(&self, painter: &Painter, rect: Rect, game: &ParadiseWheelState) {
        painter.rect_filled(rect, 10.0, Color32::from_rgb(240, 230, 190));

        let center = rect.center();
        let radius = rect.height() * 0.32;

        painter.circle_filled(center, radius, Color32::from_rgb(250, 250, 240));
        painter.circle_stroke(
            center,
            radius,
            Stroke::new(3.0_f32, Color32::from_rgb(60, 50, 40)),
        );

        let a_min = game.target_min.to_radians();
        let a_max = game.target_max.to_radians();
        let p_min = pos2(
            center.x + a_min.cos() * radius,
            center.y + a_min.sin() * radius,
        );
        let p_max = pos2(
            center.x + a_max.cos() * radius,
            center.y + a_max.sin() * radius,
        );
        painter.circle_filled(p_min, 8.0, Color32::from_rgb(60, 180, 80));
        painter.circle_filled(p_max, 8.0, Color32::from_rgb(60, 180, 80));

        let cur_rad = game.current_angle.to_radians();
        let needle_tip = pos2(
            center.x + cur_rad.cos() * (radius - 4.0),
            center.y + cur_rad.sin() * (radius - 4.0),
        );
        painter.line_segment(
            [center, needle_tip],
            Stroke::new(4.0_f32, Color32::from_rgb(220, 50, 60)),
        );
        painter.circle_filled(center, 6.0, Color32::from_rgb(40, 40, 40));
    }

    fn draw_pixel_matrix(
        &self,
        painter: &Painter,
        top_left: Pos2,
        pixels: &[Vec<Color32>],
        pixel_size: f32,
    ) {
        for (y, row) in pixels.iter().enumerate() {
            for (x, &col) in row.iter().enumerate() {
                if col != Color32::TRANSPARENT {
                    let p = pos2(
                        top_left.x + x as f32 * pixel_size,
                        top_left.y + y as f32 * pixel_size,
                    );
                    let pixel_rect = Rect::from_min_size(p, vec2(pixel_size, pixel_size));
                    painter.rect_filled(pixel_rect, 0.0, col);
                }
            }
        }
    }
}
