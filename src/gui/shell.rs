use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellColor {
    OceanBlue,
    JungleGreen,
    SunsetPink,
    CyberGrey,
}

impl ShellColor {
    pub fn palette(&self) -> (Color32, Color32, Color32) {
        match self {
            ShellColor::OceanBlue => (
                Color32::from_rgb(70, 160, 230),  // Body
                Color32::from_rgb(40, 110, 180),  // Shadow / Bezel
                Color32::from_rgb(255, 215, 0),   // Accent / Buttons
            ),
            ShellColor::JungleGreen => (
                Color32::from_rgb(85, 190, 120),
                Color32::from_rgb(50, 130, 80),
                Color32::from_rgb(255, 230, 80),
            ),
            ShellColor::SunsetPink => (
                Color32::from_rgb(240, 120, 160),
                Color32::from_rgb(180, 60, 100),
                Color32::from_rgb(255, 240, 120),
            ),
            ShellColor::CyberGrey => (
                Color32::from_rgb(80, 85, 95),
                Color32::from_rgb(50, 55, 65),
                Color32::from_rgb(100, 220, 255),
            ),
        }
    }
}

pub struct ShellControls {
    pub btn_a_clicked: bool,
    pub btn_b_clicked: bool,
    pub btn_c_clicked: bool,
    pub dial_delta: f32,
}

pub struct VirtualShell {
    pub color_theme: ShellColor,
    pub dial_angle: f32,
    pub is_dragging_dial: bool,
}

impl Default for VirtualShell {
    fn default() -> Self {
        Self {
            color_theme: ShellColor::OceanBlue,
            dial_angle: 0.0,
            is_dragging_dial: false,
        }
    }
}

impl VirtualShell {
    pub fn render(&mut self, ui: &mut Ui, available_rect: Rect) -> (Rect, ShellControls) {
        let (body_col, shadow_col, accent_col) = self.color_theme.palette();

        let mut controls = ShellControls {
            btn_a_clicked: false,
            btn_b_clicked: false,
            btn_c_clicked: false,
            dial_delta: 0.0,
        };

        // Determine shell egg bounding box
        let shell_width = (available_rect.width() * 0.88).min(380.0);
        let shell_height = (available_rect.height() * 0.92).min(520.0);
        let shell_center = available_rect.center();
        let shell_rect = Rect::from_center_size(shell_center, vec2(shell_width, shell_height));

        // 1. Draw Keychain loop on top
        let loop_center = pos2(shell_center.x, shell_rect.min.y + 12.0);
        ui.painter().circle_filled(loop_center, 18.0, shadow_col);
        ui.painter().circle_filled(loop_center, 8.0, Color32::from_rgb(240, 240, 240));

        // 2. Draw Egg Shell Body with smooth shading
        ui.painter().rect_filled(shell_rect, shell_width * 0.45, body_col);
        ui.painter().rect_stroke(
            shell_rect,
            shell_width * 0.45,
            Stroke::new(6.0_f32, shadow_col),
        );

        // 3. Gold Title Badge
        let badge_pos = pos2(shell_center.x, shell_rect.min.y + 42.0);
        ui.painter().text(
            badge_pos,
            egui::Align2::CENTER_CENTER,
            "TAMAGOTCHI PARADISE",
            egui::FontId::proportional(14.0),
            Color32::from_rgb(255, 240, 180),
        );

        // 4. LCD Screen Area
        let screen_size = (shell_width * 0.72).min(240.0);
        let screen_center = pos2(shell_center.x, shell_center.y - 25.0);
        let screen_rect = Rect::from_center_size(screen_center, vec2(screen_size, screen_size));

        // Inner screen frame
        ui.painter().rect_filled(
            screen_rect.expand(12.0),
            16.0,
            Color32::from_rgb(235, 235, 240),
        );
        ui.painter().rect_stroke(
            screen_rect.expand(12.0),
            16.0,
            Stroke::new(3.0_f32, shadow_col),
        );

        // 5. Physical Buttons (A, B, C)
        let btn_y = shell_center.y + 145.0;
        let btn_radius = 20.0;
        let spacing = 55.0;

        let btn_a_pos = pos2(shell_center.x - spacing, btn_y);
        let btn_b_pos = pos2(shell_center.x, btn_y + 12.0);
        let btn_c_pos = pos2(shell_center.x + spacing, btn_y);

        // Button A
        let btn_a_rect = Rect::from_center_size(btn_a_pos, vec2(btn_radius * 2.0, btn_radius * 2.0));
        let resp_a = ui.allocate_rect(btn_a_rect, Sense::click());
        let a_col = if resp_a.is_pointer_button_down_on() {
            shadow_col
        } else {
            accent_col
        };
        ui.painter().circle_filled(btn_a_pos, btn_radius, a_col);
        ui.painter().circle_stroke(btn_a_pos, btn_radius, Stroke::new(2.0_f32, shadow_col));
        ui.painter().text(btn_a_pos, egui::Align2::CENTER_CENTER, "A", egui::FontId::monospace(14.0), Color32::BLACK);
        if resp_a.clicked() {
            controls.btn_a_clicked = true;
        }

        // Button B
        let btn_b_rect = Rect::from_center_size(btn_b_pos, vec2(btn_radius * 2.0, btn_radius * 2.0));
        let resp_b = ui.allocate_rect(btn_b_rect, Sense::click());
        let b_col = if resp_b.is_pointer_button_down_on() {
            shadow_col
        } else {
            accent_col
        };
        ui.painter().circle_filled(btn_b_pos, btn_radius, b_col);
        ui.painter().circle_stroke(btn_b_pos, btn_radius, Stroke::new(2.0_f32, shadow_col));
        ui.painter().text(btn_b_pos, egui::Align2::CENTER_CENTER, "B", egui::FontId::monospace(14.0), Color32::BLACK);
        if resp_b.clicked() {
            controls.btn_b_clicked = true;
        }

        // Button C
        let btn_c_rect = Rect::from_center_size(btn_c_pos, vec2(btn_radius * 2.0, btn_radius * 2.0));
        let resp_c = ui.allocate_rect(btn_c_rect, Sense::click());
        let c_col = if resp_c.is_pointer_button_down_on() {
            shadow_col
        } else {
            accent_col
        };
        ui.painter().circle_filled(btn_c_pos, btn_radius, c_col);
        ui.painter().circle_stroke(btn_c_pos, btn_radius, Stroke::new(2.0_f32, shadow_col));
        ui.painter().text(btn_c_pos, egui::Align2::CENTER_CENTER, "C", egui::FontId::monospace(14.0), Color32::BLACK);
        if resp_c.clicked() {
            controls.btn_c_clicked = true;
        }

        // 6. Side Rotary Dial Knob on Right Border
        let dial_x = shell_rect.max.x - 6.0;
        let dial_y = shell_center.y - 20.0;
        let dial_rect = Rect::from_center_size(pos2(dial_x, dial_y), vec2(28.0, 75.0));
        let dial_resp = ui.allocate_rect(dial_rect, Sense::click_and_drag());

        // Handle mouse drag on dial
        if dial_resp.dragged() {
            let dy = dial_resp.drag_delta().y;
            self.dial_angle += dy * 4.0;
            controls.dial_delta += dy;
        }

        // Handle mouse wheel scroll over shell
        let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
        let pointer_pos = ui.input(|i| i.pointer.hover_pos().unwrap_or_default());
        if scroll_y != 0.0 && available_rect.contains(pointer_pos) {
            self.dial_angle -= scroll_y * 0.5;
            controls.dial_delta -= scroll_y * 0.1;
        }

        // Draw Rotary Dial with ridges
        ui.painter().rect_filled(dial_rect, 6.0, Color32::from_rgb(220, 225, 235));
        ui.painter().rect_stroke(dial_rect, 6.0, Stroke::new(2.5_f32, shadow_col));

        // Draw dynamic rotating ridges
        let num_ridges = 6;
        for i in 0..num_ridges {
            let ridge_phase = (self.dial_angle + i as f32 * 30.0) % 180.0;
            let offset_y = (ridge_phase / 180.0 - 0.5) * 60.0;
            let ridge_y = dial_y + offset_y;
            if ridge_y > dial_rect.min.y + 4.0 && ridge_y < dial_rect.max.y - 4.0 {
                ui.painter().line_segment(
                    [pos2(dial_rect.min.x + 3.0, ridge_y), pos2(dial_rect.max.x - 3.0, ridge_y)],
                    Stroke::new(2.0_f32, Color32::from_rgb(100, 110, 130)),
                );
            }
        }

        (screen_rect, controls)
    }
}
