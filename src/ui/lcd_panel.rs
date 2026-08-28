use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};

use crate::emulator::DisplayController;
use crate::gui::ShellColor;

pub struct LcdPanel;

impl LcdPanel {
    pub fn render(
        ui: &mut Ui,
        available_rect: Rect,
        display: &DisplayController,
        shell_color: ShellColor,
        on_btn_a: impl FnOnce(bool),
        on_btn_b: impl FnOnce(bool),
        on_btn_c: impl FnOnce(bool),
        on_dial_delta: impl FnOnce(i32),
        on_dial_press: impl FnOnce(bool),
    ) {
        let (body_col, shadow_col, accent_col) = shell_color.palette();

        // Calculate shell size
        let shell_width = (available_rect.width() * 0.88).min(380.0);
        let shell_height = (available_rect.height() * 0.92).min(520.0);
        let shell_center = available_rect.center();
        let shell_rect = Rect::from_center_size(shell_center, vec2(shell_width, shell_height));

        let painter = ui.painter();

        // 1. Keychain loop
        let loop_center = pos2(shell_center.x, shell_rect.min.y + 12.0);
        painter.circle_filled(loop_center, 18.0, shadow_col);
        painter.circle_filled(loop_center, 8.0, Color32::from_rgb(240, 240, 240));

        // 2. Shell Body
        painter.rect_filled(shell_rect, shell_width * 0.45, body_col);
        painter.rect_stroke(
            shell_rect,
            shell_width * 0.45,
            Stroke::new(6.0_f32, shadow_col),
        );

        // 3. Title badge
        let badge_pos = pos2(shell_center.x, shell_rect.min.y + 42.0);
        painter.text(
            badge_pos,
            egui::Align2::CENTER_CENTER,
            "TAMAGOTCHI PARADISE (SNC73410)",
            egui::FontId::proportional(13.0),
            Color32::from_rgb(255, 240, 180),
        );

        // 4. Emulated LCD Screen Bezel
        let screen_size = (shell_width * 0.72).min(240.0);
        let screen_center = pos2(shell_center.x, shell_center.y - 25.0);
        let screen_rect = Rect::from_center_size(screen_center, vec2(screen_size, screen_size));

        painter.rect_filled(screen_rect.expand(10.0), 14.0, Color32::from_rgb(230, 230, 235));
        painter.rect_stroke(screen_rect.expand(10.0), 14.0, Stroke::new(3.0_f32, shadow_col));

        // Draw emulated VRAM Framebuffer
        let pixel_w = screen_rect.width() / (display.width as f32);
        let pixel_h = screen_rect.height() / (display.height as f32);

        let colors = display.get_rgba_buffer();
        for y in 0..display.height {
            for x in 0..display.width {
                let idx = y * display.width + x;
                if idx < colors.len() {
                    let col = colors[idx];
                    let px = screen_rect.min.x + (x as f32) * pixel_w;
                    let py = screen_rect.min.y + (y as f32) * pixel_h;
                    let pixel_rect = Rect::from_min_size(pos2(px, py), vec2(pixel_w, pixel_h));
                    painter.rect_filled(pixel_rect, 0.0, col);
                }
            }
        }

        // Inner screen stroke
        painter.rect_stroke(screen_rect, 2.0, Stroke::new(1.5_f32, Color32::from_rgb(80, 80, 80)));

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
        let a_col = if resp_a.is_pointer_button_down_on() { shadow_col } else { accent_col };
        ui.painter().circle_filled(btn_a_pos, btn_radius, a_col);
        ui.painter().circle_stroke(btn_a_pos, btn_radius, Stroke::new(2.0_f32, shadow_col));
        ui.painter().text(btn_a_pos, egui::Align2::CENTER_CENTER, "A", egui::FontId::monospace(14.0), Color32::BLACK);
        on_btn_a(resp_a.is_pointer_button_down_on() || resp_a.clicked());

        // Button B
        let btn_b_rect = Rect::from_center_size(btn_b_pos, vec2(btn_radius * 2.0, btn_radius * 2.0));
        let resp_b = ui.allocate_rect(btn_b_rect, Sense::click());
        let b_col = if resp_b.is_pointer_button_down_on() { shadow_col } else { accent_col };
        ui.painter().circle_filled(btn_b_pos, btn_radius, b_col);
        ui.painter().circle_stroke(btn_b_pos, btn_radius, Stroke::new(2.0_f32, shadow_col));
        ui.painter().text(btn_b_pos, egui::Align2::CENTER_CENTER, "B", egui::FontId::monospace(14.0), Color32::BLACK);
        on_btn_b(resp_b.is_pointer_button_down_on() || resp_b.clicked());

        // Button C
        let btn_c_rect = Rect::from_center_size(btn_c_pos, vec2(btn_radius * 2.0, btn_radius * 2.0));
        let resp_c = ui.allocate_rect(btn_c_rect, Sense::click());
        let c_col = if resp_c.is_pointer_button_down_on() { shadow_col } else { accent_col };
        ui.painter().circle_filled(btn_c_pos, btn_radius, c_col);
        ui.painter().circle_stroke(btn_c_pos, btn_radius, Stroke::new(2.0_f32, shadow_col));
        ui.painter().text(btn_c_pos, egui::Align2::CENTER_CENTER, "C", egui::FontId::monospace(14.0), Color32::BLACK);
        on_btn_c(resp_c.is_pointer_button_down_on() || resp_c.clicked());

        // 6. Side Rotary Dial
        let dial_x = shell_rect.max.x - 6.0;
        let dial_y = shell_center.y - 20.0;
        let dial_rect = Rect::from_center_size(pos2(dial_x, dial_y), vec2(28.0, 75.0));
        let dial_resp = ui.allocate_rect(dial_rect, Sense::click_and_drag());

        let mut delta = 0;
        if dial_resp.dragged() {
            let dy = dial_resp.drag_delta().y;
            if dy.abs() > 2.0 {
                delta += if dy > 0.0 { 1 } else { -1 };
            }
        }

        let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_y != 0.0 && available_rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
            delta += if scroll_y > 0.0 { 1 } else { -1 };
        }

        ui.painter().rect_filled(dial_rect, 6.0, Color32::from_rgb(220, 225, 235));
        ui.painter().rect_stroke(dial_rect, 6.0, Stroke::new(2.5_f32, shadow_col));

        // La molette se presse autant qu'elle se tourne : c'est cet appui, en
        // P0.8, qui valide dans les menus du jeu.
        let ok_pos = pos2(dial_x - 4.0, dial_y + 62.0);
        let ok_rect = Rect::from_center_size(ok_pos, vec2(btn_radius * 2.2, btn_radius * 2.2));
        let resp_ok = ui.allocate_rect(ok_rect, Sense::click());
        let ok_col = if resp_ok.is_pointer_button_down_on() { shadow_col } else { accent_col };
        ui.painter().circle_filled(ok_pos, btn_radius * 1.1, ok_col);
        ui.painter().circle_stroke(ok_pos, btn_radius * 1.1, Stroke::new(2.0_f32, shadow_col));
        ui.painter().text(
            ok_pos,
            egui::Align2::CENTER_CENTER,
            "OK",
            egui::FontId::monospace(12.0),
            Color32::BLACK,
        );

        on_dial_delta(delta);
        on_dial_press(resp_ok.is_pointer_button_down_on() || resp_ok.clicked());
    }
}
