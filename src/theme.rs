//! Amber accent colours for both the dark and the light theme.

use eframe::egui::{self, Color32, Stroke, Theme};

struct Palette {
    selection: Color32,
    on_selection: Color32,
    text: Color32,
    inactive: Color32,
    hovered: Color32,
    active: Color32,
    outline: Color32,
}

const DARK: Palette = Palette {
    selection: Color32::from_rgb(0xD0, 0x8A, 0x18),
    on_selection: Color32::from_rgb(0x1A, 0x12, 0x06),
    text: Color32::from_rgb(0xF2, 0xDC, 0xB0),
    inactive: Color32::from_rgb(0x3C, 0x2C, 0x16),
    hovered: Color32::from_rgb(0x58, 0x40, 0x1A),
    active: Color32::from_rgb(0x7A, 0x58, 0x1E),
    outline: Color32::from_rgb(0x8A, 0x60, 0x1C),
};

const LIGHT: Palette = Palette {
    selection: Color32::from_rgb(0xF0, 0xA0, 0x20),
    on_selection: Color32::from_rgb(0x2A, 0x1A, 0x00),
    text: Color32::from_rgb(0x3A, 0x26, 0x00),
    inactive: Color32::from_rgb(0xF6, 0xE0, 0xB0),
    hovered: Color32::from_rgb(0xF0, 0xC8, 0x70),
    active: Color32::from_rgb(0xE8, 0xAE, 0x40),
    outline: Color32::from_rgb(0xC8, 0x86, 0x10),
};

pub fn apply(ctx: &egui::Context) {
    for (theme, p) in [(Theme::Dark, &DARK), (Theme::Light, &LIGHT)] {
        ctx.style_mut_of(theme, |style| {
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(10.0, 4.0);
            style.spacing.interact_size.x = 80.0;

            let v = &mut style.visuals;
            v.selection.bg_fill = p.selection;
            v.selection.stroke = Stroke::new(1.0, p.on_selection);
            v.hyperlink_color = p.selection;
            v.text_cursor.stroke = Stroke::new(2.0, p.selection);
            v.slider_trailing_fill = true;

            for (w, fill) in [
                (&mut v.widgets.inactive, p.inactive),
                (&mut v.widgets.hovered, p.hovered),
                (&mut v.widgets.active, p.active),
                (&mut v.widgets.open, p.hovered),
            ] {
                w.weak_bg_fill = fill;
                w.bg_fill = fill;
                w.fg_stroke = Stroke::new(1.0, p.text);
            }
            v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.outline);
            v.widgets.active.bg_stroke = Stroke::new(1.0, p.selection);
        });
    }
}
