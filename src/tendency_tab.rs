//! World Tendency tab: one card per world plus the character, in a 2×3 grid.

use eframe::egui;

use crate::save::Tendencies;

const RANGE: std::ops::RangeInclusive<f32> = -200.0..=200.0;
const GAP: f32 = 12.0;
const CARD_PADDING: f32 = 12.0;

pub fn show(ui: &mut egui::Ui, edited: &mut Tendencies, original: &Tendencies) {
    ui.heading("World Tendency");
    ui.add_space(8.0);

    let o = *original;
    let cards: [(&str, &mut f32, f32); 6] = [
        ("Character Tendency", &mut edited.character, o.character),
        (
            "1 · Boletarian Palace",
            &mut edited.boletarian_palace,
            o.boletarian_palace,
        ),
        (
            "2 · Stonefang Tunnel",
            &mut edited.stonefang_tunnel,
            o.stonefang_tunnel,
        ),
        (
            "3 · Tower of Latria",
            &mut edited.tower_of_latria,
            o.tower_of_latria,
        ),
        (
            "4 · Shrine of Storms",
            &mut edited.shrine_of_storms,
            o.shrine_of_storms,
        ),
        (
            "5 · Valley of Defilement",
            &mut edited.valley_of_defilement,
            o.valley_of_defilement,
        ),
    ];

    ui.spacing_mut().item_spacing.x = GAP;
    let mut cards = cards.into_iter();
    for _ in 0..2 {
        ui.columns(3, |columns| {
            for (column, (title, value, original)) in columns.iter_mut().zip(cards.by_ref()) {
                card(column, title, value, original);
            }
        });
        ui.add_space(GAP);
    }
}

fn card(ui: &mut egui::Ui, title: &str, value: &mut f32, original: f32) {
    // Leave room for the frame's padding and 1px stroke on each side.
    let inner = (ui.available_width() - 2.0 * (CARD_PADDING + 1.0)).floor();
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(8)
        .inner_margin(CARD_PADDING)
        .show(ui, |ui| {
            ui.set_width(inner);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.strong(title);
                    if *value != original {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.weak(format!("was {original}"));
                        });
                    }
                });
                ui.add_space(4.0);

                // The slider's value box is `interact_size.x` wide, after one item gap.
                let value_box = ui.spacing().interact_size.x + ui.spacing().item_spacing.x;
                ui.spacing_mut().slider_width = (inner - value_box).max(60.0);
                ui.add(
                    egui::Slider::new(value, RANGE)
                        .step_by(1.0)
                        .clamping(egui::SliderClamping::Edits),
                );
            });
        });
}
