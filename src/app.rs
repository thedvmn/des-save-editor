use std::fs;
use std::path::{Path, PathBuf};

use eframe::egui;

use crate::item_list::ItemList;
use crate::save::{self, Character, SLOT_COUNT, SaveDir, Stats, inventory, stockpile};
use crate::settings::Settings;
use crate::tendency_tab;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Levels,
    Inventory,
    Thomas,
    WorldTendency,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Levels, Tab::Inventory, Tab::Thomas, Tab::WorldTendency];

    fn label(self) -> &'static str {
        match self {
            Tab::Levels => "Levels",
            Tab::Inventory => "Inventory",
            Tab::Thomas => "Thomas",
            Tab::WorldTendency => "World Tendency",
        }
    }
}

/// One character slot read from disk, plus the user's pending edits.
struct LoadedSlot {
    slot: u8,
    path: PathBuf,
    /// File contents as last read from / written to disk.
    bytes: Vec<u8>,
    original: Character,
    edited: Character,
}

impl LoadedSlot {
    fn is_dirty(&self) -> bool {
        self.edited != self.original
    }
}

#[derive(Default)]
enum Status {
    #[default]
    None,
    Info(String),
    Error(String),
}

pub struct SaveEditorApp {
    dirs: Vec<SaveDir>,
    dir_idx: usize,
    slots: Vec<LoadedSlot>,
    slot_idx: usize,
    tab: Tab,
    confirm_save: bool,
    /// Ask whether to back up the folder that was just loaded.
    backup_prompt: bool,
    /// State of the prompt's "Don't show again" checkbox.
    dont_show_again: bool,
    status: Status,
    settings: Settings,
    inventory_list: ItemList,
    stockpile_list: ItemList,
}

impl Default for SaveEditorApp {
    fn default() -> Self {
        Self {
            dirs: Vec::new(),
            dir_idx: 0,
            slots: Vec::new(),
            slot_idx: 0,
            tab: Tab::default(),
            confirm_save: false,
            backup_prompt: false,
            dont_show_again: false,
            status: Status::default(),
            settings: Settings::default(),
            inventory_list: ItemList::new("inventory"),
            stockpile_list: ItemList::new("stockpile"),
        }
    }
}

impl SaveEditorApp {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            ..Default::default()
        }
    }

    fn has_unsaved(&self) -> bool {
        self.slots.iter().any(LoadedSlot::is_dirty)
    }

    pub fn open_folder(&mut self, path: &Path) {
        match save::find_save_dirs(path) {
            Ok(dirs) if dirs.is_empty() => {
                self.status = Status::Error(format!(
                    "No Demon's Souls save (PARAM.SFO) found in {}",
                    path.display()
                ));
            }
            Ok(dirs) => {
                self.dirs = dirs;
                self.select_dir(0);
            }
            Err(e) => {
                self.status = Status::Error(format!("Could not read {}: {e}", path.display()))
            }
        }
    }

    fn select_dir(&mut self, idx: usize) {
        self.dir_idx = idx;
        self.slots.clear();
        self.slot_idx = 0;

        let dir = &self.dirs[idx];
        let mut errors = Vec::new();
        for slot in 1..=SLOT_COUNT {
            let Some(path) = dir.active_slot_file(slot) else {
                continue;
            };
            let bytes = match fs::read(path) {
                Ok(b) => b,
                Err(e) => {
                    errors.push(format!("slot {slot}: {e}"));
                    continue;
                }
            };
            match Character::read(&bytes) {
                Ok(c) => self.slots.push(LoadedSlot {
                    slot,
                    path: path.to_owned(),
                    bytes,
                    original: c.clone(),
                    edited: c,
                }),
                Err(save::user_dat::UserDatError::EmptySlot) => {}
                Err(e) => errors.push(format!("slot {slot}: {e}")),
            }
        }

        self.status = if !errors.is_empty() {
            Status::Error(errors.join("; "))
        } else if self.slots.is_empty() {
            Status::Error("This save folder has no characters.".to_owned())
        } else {
            Status::Info(format!("Loaded {}", dir.path.display()))
        };
        self.backup_prompt = !self.slots.is_empty() && self.settings.backup_prompt;
    }

    /// Asks where to put a backup of the current save folder and makes it.
    fn create_backup(&mut self) {
        let Some(dir) = self.dirs.get(self.dir_idx) else {
            return;
        };
        let mut dialog = rfd::FileDialog::new().set_title("Choose where to put the backup");
        if let Some(last) = &self.settings.last_backup_dir {
            dialog = dialog.set_directory(last);
        }
        let Some(target) = dialog.pick_folder() else {
            return;
        };

        // A copy inside RPCS3's savedata folder would show up in-game as a
        // second save, and one inside the save folder would be copied along
        // with it next time.
        if target.starts_with(&dir.path) || dir.path.parent() == Some(target.as_path()) {
            self.status = Status::Error(
                "Choose a folder outside the save folder and RPCS3's savedata folder.".to_owned(),
            );
            return;
        }

        self.status = match save::backup_save_dir(&dir.path, &target) {
            Ok(backup) => Status::Info(format!("Backup created: {}", backup.display())),
            Err(e) => Status::Error(format!("Backup failed: {e}")),
        };
        self.settings.last_backup_dir = Some(target);
        self.save_settings();
    }

    fn save_settings(&mut self) {
        if let Err(e) = self.settings.save() {
            self.status = Status::Error(format!("Could not save settings: {e}"));
        }
    }

    fn revert(&mut self) {
        for s in &mut self.slots {
            s.edited = s.original.clone();
        }
        self.status = Status::Info("Changes reverted.".to_owned());
    }

    fn save(&mut self) {
        self.status = match self.try_save() {
            Ok(msg) => Status::Info(msg),
            Err(msg) => Status::Error(msg),
        };
    }

    fn try_save(&mut self) -> Result<String, String> {
        // Refuse to overwrite anything that changed since we loaded it
        // (e.g. the game saved in the meantime).
        for s in self.slots.iter().filter(|s| s.is_dirty()) {
            let on_disk = fs::read(&s.path)
                .map_err(|e| format!("Could not read {}: {e}", s.path.display()))?;
            if on_disk != s.bytes {
                return Err(format!(
                    "{} changed on disk since it was loaded. Re-open the folder and try again.",
                    s.path.display()
                ));
            }
        }

        let mut written = 0;
        for s in self.slots.iter_mut().filter(|s| s.is_dirty()) {
            let mut bytes = s.bytes.clone();
            s.edited.write_into(&mut bytes).map_err(|e| e.to_string())?;
            save::write_atomic(&s.path, &bytes)
                .map_err(|e| format!("Writing {} failed: {e}", s.path.display()))?;
            // Re-read so newly added items pick up the slots they were given.
            s.original = Character::read(&bytes).map_err(|e| e.to_string())?;
            s.edited = s.original.clone();
            s.bytes = bytes;
            written += 1;
        }

        Ok(format!("Saved {written} slot(s)."))
    }

    // ---- UI ----

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let unsaved = self.has_unsaved();
        ui.horizontal(|ui| {
            let open = ui
                .add_enabled(!unsaved, egui::Button::new("Open folder…"))
                .on_disabled_hover_text("Save or revert your changes first");
            if open.clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select save folder")
                    .pick_folder()
                {
                    self.open_folder(&path);
                }
            }

            if self.dirs.len() > 1 {
                let mut idx = self.dir_idx;
                ui.add_enabled_ui(!unsaved, |ui| {
                    egui::ComboBox::from_id_salt("save_dir")
                        .selected_text(self.dirs[idx].directory_name())
                        .show_ui(ui, |ui| {
                            for (i, d) in self.dirs.iter().enumerate() {
                                ui.selectable_value(&mut idx, i, d.directory_name());
                            }
                        });
                });
                if idx != self.dir_idx {
                    self.select_dir(idx);
                }
            }

            if !self.slots.is_empty() {
                let label = |s: &LoadedSlot| {
                    let mark = if s.is_dirty() { " •" } else { "" };
                    format!("Slot {} — {}{mark}", s.slot, s.original.name)
                };
                egui::ComboBox::from_id_salt("slot")
                    .selected_text(label(&self.slots[self.slot_idx]))
                    .show_ui(ui, |ui| {
                        for (i, s) in self.slots.iter().enumerate() {
                            ui.selectable_value(&mut self.slot_idx, i, label(s));
                        }
                    });
            }

            if let Some(dir) = self.dirs.get(self.dir_idx) {
                ui.weak(dir.path.display().to_string());
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let height = ui.spacing().interact_size.y;
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            egui::vec2(width, height),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                let unsaved = self.has_unsaved();
                if ui
                    .add_enabled(unsaved, egui::Button::new("Save…"))
                    .clicked()
                {
                    self.confirm_save = true;
                }
                if ui
                    .add_enabled(unsaved, egui::Button::new("Revert"))
                    .clicked()
                {
                    self.revert();
                }
                let backup = ui
                    .add_enabled(!self.dirs.is_empty(), egui::Button::new("Create Backup…"))
                    .on_hover_text(
                        "Copy the save folder as it is on disk (unsaved changes aren't included)",
                    );
                if backup.clicked() {
                    self.create_backup();
                }

                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let text = match &self.status {
                        Status::None => return,
                        Status::Info(msg) => egui::RichText::new(msg),
                        Status::Error(msg) => {
                            egui::RichText::new(msg).color(ui.visuals().error_fg_color)
                        }
                    };
                    ui.add(egui::Label::new(text).truncate());
                });
            },
        );
    }

    fn tab_list(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        for tab in Tab::ALL {
            let button = egui::Button::selectable(self.tab == tab, tab.label())
                .min_size(egui::vec2(width, 32.0));
            if ui.add(button).clicked() {
                self.tab = tab;
            }
        }
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::unique("confirm_save")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading("Write changes to the save?");
            ui.add_space(6.0);
            ui.label("These files will be overwritten:");
            for s in self.slots.iter().filter(|s| s.is_dirty()) {
                ui.monospace(s.path.display().to_string());
            }
            ui.add_space(6.0);
            ui.label("No backup is made automatically. Use Create Backup… first if you want one.");
            ui.add_space(6.0);
            ui.strong("Make sure the game isn't running in RPCS3.");
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    choice = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(false);
                }
            });
        });

        if modal.should_close() && choice.is_none() {
            choice = Some(false);
        }
        if let Some(confirmed) = choice {
            self.confirm_save = false;
            if confirmed {
                self.save();
            }
        }
    }
}

impl SaveEditorApp {
    fn backup_prompt_dialog(&mut self, ctx: &egui::Context) {
        let Some(dir) = self.dirs.get(self.dir_idx) else {
            return;
        };
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::unique("backup_prompt")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading("Back up this save folder?");
            ui.add_space(6.0);
            ui.label("Make a copy of the save folder before you change anything:");
            ui.monospace(dir.directory_name());
            ui.add_space(6.0);
            ui.checkbox(&mut self.dont_show_again, "Don't show again");
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Back up…").clicked() {
                    choice = Some(true);
                }
                if ui.button("Not now").clicked() {
                    choice = Some(false);
                }
            });
        });

        if modal.should_close() && choice.is_none() {
            choice = Some(false);
        }
        if let Some(back_up) = choice {
            self.backup_prompt = false;
            if self.dont_show_again {
                self.settings.backup_prompt = false;
                self.save_settings();
            }
            if back_up {
                self.create_backup();
            }
        }
    }
}

/// Shows "(was N)" next to a field whose value differs from the original.
fn was<T: PartialEq + std::fmt::Display>(ui: &mut egui::Ui, current: T, original: T) {
    if current != original {
        ui.weak(format!("(was {original})"));
    } else {
        ui.label("");
    }
}

fn levels_tab(ui: &mut egui::Ui, slot: &mut LoadedSlot) {
    ui.heading("Levels");
    ui.add_space(8.0);

    let original = slot.original.stats.as_array();
    egui::Grid::new("stats")
        .num_columns(3)
        .spacing([24.0, 8.0])
        .show(ui, |ui| {
            for ((name, value), orig) in Stats::NAMES
                .iter()
                .zip(slot.edited.stats.as_array_mut())
                .zip(original)
            {
                ui.label(*name);
                ui.spacing_mut().slider_width = 280.0;
                ui.add(egui::Slider::new(value, 1..=99).clamping(egui::SliderClamping::Edits));
                was(ui, *value, orig);
                ui.end_row();
            }
        });

    // Every stat point is one purchased level, so keep the counter in step.
    let sum = |s: [u32; 8]| s.iter().map(|&v| i64::from(v)).sum::<i64>();
    let delta = sum(slot.edited.stats.as_array()) - sum(original);
    slot.edited.levels_purchased =
        (i64::from(slot.original.levels_purchased) + delta).max(0) as u32;

    ui.add_space(12.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(format!(
            "Levels purchased: {}",
            slot.edited.levels_purchased
        ));
        if delta != 0 {
            ui.weak(format!("({delta:+})"));
        }
    });
}

impl eframe::App for SaveEditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // eframe only repaints on input by default; nothing here requests
        // continuous repaints.
        let bar_frame =
            egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::symmetric(10, 8));
        egui::Panel::top("toolbar")
            .frame(bar_frame)
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status")
            .frame(bar_frame)
            .show(ui, |ui| self.status_bar(ui));

        if self.slots.is_empty() {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.weak("Open a Demon's Souls save folder to begin.");
                });
            });
            return;
        }

        let side_frame =
            egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::same(8));
        egui::Panel::left("tabs")
            .resizable(false)
            .exact_size(170.0)
            .frame(side_frame)
            .show(ui, |ui| self.tab_list(ui));

        let slot = &mut self.slots[self.slot_idx];
        let central_frame =
            egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin::same(14));
        egui::CentralPanel::default()
            .frame(central_frame)
            .show(ui, |ui| match self.tab {
                Tab::Levels => {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| levels_tab(ui, slot));
                }
                // The item tabs have their own scrolling lists.
                Tab::Inventory => {
                    ui.heading("Inventory");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Souls");
                        ui.add(
                            egui::DragValue::new(&mut slot.edited.souls)
                                .range(0..=999_999_999)
                                .clamp_existing_to_range(false)
                                .speed(50.0),
                        );
                        if slot.edited.souls != slot.original.souls {
                            ui.weak(format!("was {}", slot.original.souls));
                        }
                    });
                    ui.add_space(8.0);
                    self.inventory_list.show(
                        ui,
                        "Items",
                        &mut slot.edited.inventory,
                        &slot.original.inventory,
                        |items, def, count| inventory::add(items, def, count),
                    );
                }
                Tab::Thomas => {
                    ui.heading("Stockpile Thomas");
                    ui.add_space(8.0);
                    let inventory = &slot.edited.inventory;
                    self.stockpile_list.show(
                        ui,
                        "Stored items",
                        &mut slot.edited.stockpile,
                        &slot.original.stockpile,
                        |items, def, count| {
                            // Reuse the inventory's menu sort ID for the same item.
                            let sort_id = inventory
                                .iter()
                                .find(|i| i.category == def.category && i.id == def.id)
                                .map(|i| i.sort_id());
                            stockpile::add(items, def, count, sort_id);
                        },
                    );
                }
                Tab::WorldTendency => {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            tendency_tab::show(
                                ui,
                                &mut slot.edited.tendencies,
                                &slot.original.tendencies,
                            );
                        });
                }
            });

        if self.backup_prompt {
            let ctx = ui.ctx().clone();
            self.backup_prompt_dialog(&ctx);
        }
        if self.confirm_save {
            let ctx = ui.ctx().clone();
            self.confirm_dialog(&ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for e in fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }

    /// Opens a scratch copy of the real save in `.save-data/`, edits it,
    /// saves, and checks exactly which bytes changed.
    #[test]
    fn save_roundtrip_on_real_save() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join(".save-data/BLES00932DEMONSS005");
        if !src.is_dir() {
            eprintln!("skipping: no .save-data");
            return;
        }
        let tmp = std::env::temp_dir().join(format!("des-save-editor-app-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let save_dir = tmp.join("BLES00932DEMONSS005");
        copy_dir(&src, &save_dir);

        let mut app = SaveEditorApp::default();
        app.open_folder(&tmp);
        assert_eq!(app.slots.len(), 1, "expected one character");
        assert!(app.backup_prompt, "loading a folder asks about a backup");
        let original_file = fs::read(save_dir.join("USER.DAT")).unwrap();

        let slot = &mut app.slots[0];
        slot.edited.stats.strength += 2;
        slot.edited.levels_purchased += 2;
        slot.edited.souls = 50_000;
        slot.edited.tendencies.tower_of_latria = -30.0;
        app.try_save().unwrap();
        assert!(!app.has_unsaved());

        // Saving doesn't make backups on its own.
        assert_eq!(fs::read_dir(&tmp).unwrap().count(), 1);

        // Only the expected regions changed.
        let new_file = fs::read(save_dir.join("USER.DAT")).unwrap();
        let changed: Vec<usize> = (0..new_file.len())
            .filter(|&i| new_file[i] != original_file[i])
            .collect();
        let allowed = |i: usize| {
            (0xBC..0xC0).contains(&i)          // souls
                || (0x94..0x9C).contains(&i)   // strength base + effective
                || (0xCC..0xD0).contains(&i)   // levels purchased
                || (0x1EC10..0x1EC18).contains(&i) // Latria tendency + mirror
        };
        assert!(!changed.is_empty());
        assert!(
            changed.iter().all(|&i| allowed(i)),
            "unexpected changes at {changed:x?}"
        );

        // Re-reading gives the edited values; other slot files untouched.
        let c = Character::read(&new_file).unwrap();
        assert_eq!(c, app.slots[0].edited);
        assert_eq!(
            fs::read(save_dir.join("2USER.DAT")).unwrap(),
            fs::read(src.join("2USER.DAT")).unwrap()
        );

        // Inventory: remove, change a stack, stack onto an existing item, add new.
        use crate::save::inventory;
        use crate::save::items::{self, ItemCategory};
        let inv = &mut app.slots[0].edited.inventory;
        let before = inv.len();
        assert!(
            inv.iter()
                .find(|i| i.name() == "Silver Catalyst")
                .unwrap()
                .equipped
        );
        let talisman = inv
            .iter()
            .position(|i| i.name() == "Talisman of God")
            .unwrap();
        let freed_slot = inv[talisman].slot;
        inv.remove(talisman);
        inv.iter_mut()
            .find(|i| i.name() == "Old Spice")
            .unwrap()
            .count = 30;
        inventory::add(inv, items::lookup(ItemCategory::Goods, 0x3E8).unwrap(), 10); // Crescent Moon Grass 64 -> 74
        inventory::add(inv, items::lookup(ItemCategory::Weapon, 0x2710).unwrap(), 1); // Dagger
        app.try_save().unwrap();

        let inv = &app.slots[0].original.inventory;
        assert_eq!(inv.len(), before);
        assert!(inv.iter().all(|i| i.name() != "Talisman of God"));
        assert_eq!(
            inv.iter().find(|i| i.name() == "Old Spice").unwrap().count,
            30
        );
        assert_eq!(
            inv.iter()
                .find(|i| i.name() == "Crescent Moon Grass")
                .unwrap()
                .count,
            74
        );
        let dagger = inv.iter().find(|i| i.name() == "Dagger").unwrap();
        assert_eq!(
            dagger.slot, freed_slot,
            "new item reuses the first free slot"
        );
        assert_eq!(dagger.durability, 200);

        // Saving again with no edits changes nothing (the dagger isn't re-added).
        let after_first = fs::read(save_dir.join("USER.DAT")).unwrap();
        app.slots[0].edited.souls += 1;
        app.try_save().unwrap();
        let after_second = fs::read(save_dir.join("USER.DAT")).unwrap();
        let diff: Vec<usize> = (0..after_first.len())
            .filter(|&i| after_first[i] != after_second[i])
            .collect();
        assert!(
            diff.iter().all(|&i| (0xBC..0xC0).contains(&i)),
            "unexpected changes at {diff:x?}"
        );

        // A file changed behind our back is not overwritten.
        app.slots[0].edited.souls = 1;
        fs::write(save_dir.join("USER.DAT"), &original_file).unwrap();
        assert!(app.try_save().is_err());

        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn stockpile_and_durability_edits_on_real_save() {
        use crate::save::items::{self, ItemCategory};

        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join(".save-data/BLES00932DEMONSS005");
        if !src.is_dir() {
            eprintln!("skipping: no .save-data");
            return;
        }
        let tmp =
            std::env::temp_dir().join(format!("des-save-editor-thomas-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let save_dir = tmp.join("BLES00932DEMONSS005");
        copy_dir(&src, &save_dir);

        let mut app = SaveEditorApp::default();
        app.open_folder(&tmp);
        let original_file = fs::read(save_dir.join("USER.DAT")).unwrap();

        let c = &mut app.slots[0].edited;
        assert_eq!(c.stockpile.len(), 65);
        let sword = c
            .stockpile
            .iter()
            .position(|i| i.name() == "Bastard Sword")
            .unwrap();
        let sword_slot = c.stockpile[sword].slot;
        c.stockpile.remove(sword);
        c.stockpile
            .iter_mut()
            .find(|i| i.name() == "Scimitar")
            .unwrap()
            .durability = 60;
        c.stockpile
            .iter_mut()
            .find(|i| i.name() == "Shard of Sharpstone")
            .unwrap()
            .count = 40;
        stockpile::add(
            &mut c.stockpile,
            items::lookup(ItemCategory::Ring, 0x71).unwrap(),
            1,
            None,
        ); // Cling Ring
        c.inventory
            .iter_mut()
            .find(|i| i.name() == "Halberd+8")
            .unwrap()
            .durability = 100;
        app.try_save().unwrap();

        let c = &app.slots[0].original;
        assert_eq!(c.stockpile.len(), 65);
        assert!(c.stockpile.iter().all(|i| i.name() != "Bastard Sword"));
        assert_eq!(
            c.stockpile
                .iter()
                .find(|i| i.name() == "Scimitar")
                .unwrap()
                .durability,
            60
        );
        assert_eq!(
            c.stockpile
                .iter()
                .find(|i| i.name() == "Shard of Sharpstone")
                .unwrap()
                .count,
            40
        );
        assert_eq!(
            c.stockpile
                .iter()
                .find(|i| i.name() == "Cling Ring")
                .unwrap()
                .slot,
            sword_slot
        );
        assert_eq!(
            c.inventory
                .iter()
                .find(|i| i.name() == "Halberd+8")
                .unwrap()
                .durability,
            100
        );

        // Changes stay inside the stockpile entries touched and the durability table.
        let new_file = fs::read(save_dir.join("USER.DAT")).unwrap();
        let entry = |slot: usize| 0x14BE8 + slot * 0x14..0x14BE8 + (slot + 1) * 0x14;
        let touched = [0usize, 1, 10];
        let changed: Vec<usize> = (0..new_file.len())
            .filter(|&i| new_file[i] != original_file[i])
            .collect();
        assert!(
            changed
                .iter()
                .all(|&i| touched.iter().any(|&s| entry(s).contains(&i))
                    || (0x10364..0x10364 + 0x800 * 8).contains(&i)),
            "unexpected changes at {changed:x?}"
        );

        fs::remove_dir_all(&tmp).unwrap();
    }
}
