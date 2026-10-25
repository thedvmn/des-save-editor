//! The item table shared by the Inventory and Thomas tabs: a filterable list
//! of what's there, plus a searchable list of items to add.

use std::collections::HashMap;

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::save::inventory::InventoryItem;
use crate::save::items::{self, ItemCategory, ItemDef};
use crate::save::stockpile::{self, StockpileItem};

const ROW_HEIGHT: f32 = 24.0;
/// Height reserved below the item list for the "Add item" section.
const ADD_SECTION_HEIGHT: f32 = 200.0;

/// What the shared table needs from an inventory or stockpile entry.
pub trait Entry {
    fn def(&self) -> Option<&'static ItemDef>;
    fn name(&self) -> String;
    fn category(&self) -> ItemCategory;
    fn slot(&self) -> Option<u16>;
    fn count(&self) -> u32;
    fn durability(&self) -> u32;
    fn count_mut(&mut self) -> &mut u32;
    fn durability_mut(&mut self) -> &mut u32;
    /// Largest count the save format can store for this entry.
    fn count_limit(&self) -> u32 {
        u32::MAX
    }
    /// Extra label shown next to the name.
    fn tag(&self) -> Option<&'static str> {
        None
    }
    /// `Err(reason)` when the entry must not be removed.
    fn removable(&self) -> Result<(), &'static str> {
        match self.def() {
            Some(d) if d.is_placeholder() => Err("The game needs this item"),
            _ => Ok(()),
        }
    }
}

impl Entry for InventoryItem {
    fn def(&self) -> Option<&'static ItemDef> {
        InventoryItem::def(self)
    }
    fn name(&self) -> String {
        InventoryItem::name(self)
    }
    fn category(&self) -> ItemCategory {
        self.category
    }
    fn slot(&self) -> Option<u16> {
        self.slot
    }
    fn count(&self) -> u32 {
        self.count
    }
    fn durability(&self) -> u32 {
        self.durability
    }
    fn count_mut(&mut self) -> &mut u32 {
        &mut self.count
    }
    fn durability_mut(&mut self) -> &mut u32 {
        &mut self.durability
    }
    fn tag(&self) -> Option<&'static str> {
        self.equipped.then_some("equipped")
    }
    fn removable(&self) -> Result<(), &'static str> {
        if self.equipped {
            return Err("Equipped items can't be removed");
        }
        match self.def() {
            Some(d) if d.is_placeholder() => Err("The game needs this item"),
            _ => Ok(()),
        }
    }
}

impl Entry for StockpileItem {
    fn def(&self) -> Option<&'static ItemDef> {
        StockpileItem::def(self)
    }
    fn name(&self) -> String {
        StockpileItem::name(self)
    }
    fn category(&self) -> ItemCategory {
        self.category
    }
    fn slot(&self) -> Option<u16> {
        self.slot
    }
    fn count(&self) -> u32 {
        self.count
    }
    fn durability(&self) -> u32 {
        self.durability
    }
    fn count_mut(&mut self) -> &mut u32 {
        &mut self.count
    }
    fn durability_mut(&mut self) -> &mut u32 {
        &mut self.durability
    }
    fn count_limit(&self) -> u32 {
        stockpile::MAX_COUNT
    }
}

/// Filter and search state for one item table.
pub struct ItemList {
    id: &'static str,
    filter: Option<ItemCategory>,
    search: String,
    add_category: ItemCategory,
    add_search: String,
    add_count: u32,
    last_added: Option<&'static str>,
}

fn matches_search(name: &str, search: &str) -> bool {
    search.is_empty() || name.to_lowercase().contains(&search.to_lowercase())
}

impl ItemList {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            filter: None,
            search: String::new(),
            add_category: ItemCategory::Weapon,
            add_search: String::new(),
            add_count: 1,
            last_added: None,
        }
    }

    /// Shows the list of `entries` and the "Add item" section. `add` is
    /// called with the item and count the user chose to add.
    pub fn show<T: Entry>(
        &mut self,
        ui: &mut egui::Ui,
        title: &str,
        entries: &mut Vec<T>,
        original: &[T],
        add: impl FnOnce(&mut Vec<T>, &'static ItemDef, u32),
    ) {
        self.list(ui, title, entries, original);
        ui.add_space(4.0);
        ui.separator();
        if let Some((def, count)) = self.add_section(ui) {
            add(entries, def, count);
            self.last_added = Some(def.name);
        }
    }

    fn list<T: Entry>(
        &mut self,
        ui: &mut egui::Ui,
        title: &str,
        entries: &mut Vec<T>,
        original: &[T],
    ) {
        let original_values: HashMap<u16, (u32, u32)> = original
            .iter()
            .filter_map(|e| Some((e.slot()?, (e.count(), e.durability()))))
            .collect();
        let removed = original
            .iter()
            .filter(|o| !entries.iter().any(|e| e.slot() == o.slot()))
            .count();

        ui.horizontal(|ui| {
            ui.strong(title);
            let mut summary = format!("{} items", entries.len());
            if removed > 0 {
                summary += &format!(", {removed} removed");
            }
            ui.weak(summary);

            ui.add_space(12.0);
            ui.selectable_value(&mut self.filter, None, "All");
            for cat in ItemCategory::ALL {
                ui.selectable_value(&mut self.filter, Some(cat), cat.label());
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Search…")
                        .desired_width(160.0),
                );
            });
        });

        let shown: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| self.filter.is_none_or(|c| c == e.category()))
            .filter(|(_, e)| matches_search(&e.name(), &self.search))
            .map(|(n, _)| n)
            .collect();

        let height = (ui.available_height() - ADD_SECTION_HEIGHT).max(120.0);
        let mut remove = None;

        TableBuilder::new(ui)
            .id_salt((self.id, "list"))
            .striped(true)
            .auto_shrink([false, false])
            .min_scrolled_height(height)
            .max_scroll_height(height)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(180.0).clip(true))
            .column(Column::initial(130.0).clip(true))
            .column(Column::exact(80.0))
            .column(Column::exact(130.0))
            .column(Column::exact(80.0))
            .header(20.0, |mut header| {
                for title in ["Name", "Type", "Count", "Durability", ""] {
                    header.col(|ui| {
                        ui.weak(title);
                    });
                }
            })
            .body(|body| {
                body.rows(ROW_HEIGHT, shown.len(), |mut row| {
                    let n = shown[row.index()];
                    let entry = &mut entries[n];
                    let def = entry.def();
                    let was = entry.slot().and_then(|s| original_values.get(&s)).copied();

                    row.col(|ui| {
                        ui.label(entry.name());
                        if entry.slot().is_none() {
                            ui.weak("new");
                        }
                        if let Some(tag) = entry.tag() {
                            ui.weak(tag);
                        }
                    });
                    row.col(|ui| {
                        ui.weak(def.map_or(entry.category().label(), |d| d.group));
                    });
                    row.col(|ui| count_editor(ui, entry, def, was.map(|w| w.0)));
                    row.col(|ui| durability_editor(ui, entry, def, was.map(|w| w.1)));
                    row.col(|ui| {
                        let allowed = entry.removable();
                        let button =
                            ui.add_enabled(allowed.is_ok(), egui::Button::new("Remove").small());
                        if let Err(reason) = allowed {
                            button.on_disabled_hover_text(reason);
                        } else if button.clicked() {
                            remove = Some(n);
                        }
                    });
                });
            });

        if let Some(n) = remove {
            entries.remove(n);
        }
    }

    /// Returns the item and count to add when an "Add" button was clicked.
    fn add_section(&mut self, ui: &mut egui::Ui) -> Option<(&'static ItemDef, u32)> {
        ui.horizontal(|ui| {
            ui.strong("Add item");
            egui::ComboBox::from_id_salt((self.id, "add_category"))
                .selected_text(self.add_category.label())
                .show_ui(ui, |ui| {
                    for cat in ItemCategory::ALL {
                        ui.selectable_value(&mut self.add_category, cat, cat.label());
                    }
                });
            ui.add(
                egui::TextEdit::singleline(&mut self.add_search)
                    .hint_text("Search…")
                    .desired_width(160.0),
            );
            if matches!(
                self.add_category,
                ItemCategory::Goods | ItemCategory::Weapon
            ) {
                ui.label("Count");
                ui.add(
                    egui::DragValue::new(&mut self.add_count)
                        .range(1..=999)
                        .speed(0.2),
                )
                .on_hover_text("For goods and ammo; other items are always added once");
            }
            if let Some(name) = self.last_added {
                ui.weak(format!("Added {name}"));
            }
        });

        let search = self.add_search.to_lowercase();
        let candidates: Vec<&'static ItemDef> = items::all()
            .iter()
            .filter(|d| d.category == self.add_category && !d.hidden && !d.is_placeholder())
            .filter(|d| search.is_empty() || d.search_key.contains(&search))
            .collect();

        let mut add = None;
        let height = ui.available_height();
        TableBuilder::new(ui)
            .id_salt((self.id, "add"))
            .striped(true)
            .auto_shrink([false, false])
            .max_scroll_height(height)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(180.0).clip(true))
            .column(Column::initial(130.0).clip(true))
            .column(Column::exact(80.0))
            .body(|body| {
                body.rows(ROW_HEIGHT, candidates.len(), |mut row| {
                    let def = candidates[row.index()];
                    row.col(|ui| {
                        ui.label(def.name);
                    });
                    row.col(|ui| {
                        ui.weak(def.group);
                    });
                    row.col(|ui| {
                        if ui.add(egui::Button::new("Add").small()).clicked() {
                            add = Some(def);
                        }
                    });
                });
            });

        add.map(|def| (def, self.add_count))
    }
}

fn count_editor<T: Entry>(
    ui: &mut egui::Ui,
    entry: &mut T,
    def: Option<&ItemDef>,
    was: Option<u32>,
) {
    let limit = entry.count_limit();
    let count = entry.count_mut();
    match def {
        Some(d) if d.is_stackable() => {
            ui.add(
                egui::DragValue::new(count)
                    .range(1..=d.max_count().min(limit))
                    .clamp_existing_to_range(false)
                    .speed(0.2),
            );
        }
        _ => {
            ui.label(count.to_string());
        }
    }
    if let Some(w) = was.filter(|&w| w != *count) {
        ui.weak(format!("was {w}"))
            .on_hover_text("Value in the save file");
    }
}

fn durability_editor<T: Entry>(
    ui: &mut egui::Ui,
    entry: &mut T,
    def: Option<&ItemDef>,
    was: Option<u32>,
) {
    let has_durability = entry.category().has_durability() && !def.is_some_and(ItemDef::is_ammo);
    if !has_durability {
        ui.weak("—");
        return;
    }
    let durability = entry.durability_mut();
    let max = def.and_then(|d| d.max_durability).unwrap_or(*durability);
    ui.add(
        egui::DragValue::new(durability)
            .range(0..=max.max(1))
            .clamp_existing_to_range(false)
            .speed(1.0),
    );
    ui.weak(format!("/ {max}"));
    if let Some(w) = was.filter(|&w| w != *durability) {
        ui.weak(format!("was {w}"));
    }
}
