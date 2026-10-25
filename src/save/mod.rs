//! Demon's Souls (PS3 / RPCS3) save-file handling.

pub mod backup;
pub mod folder;
pub mod inventory;
pub mod items;
pub mod sfo;
pub mod stockpile;
pub mod user_dat;

pub use backup::{backup_save_dir, write_atomic};
pub use folder::{SLOT_COUNT, SaveDir, find_save_dirs};
pub use user_dat::{Character, Stats, Tendencies};

/// Tests against the real save placed in `.save-data/` (git-ignored).
/// They are skipped when that directory doesn't exist.
#[cfg(test)]
mod real_save_tests {
    use super::*;
    use std::path::PathBuf;

    fn save_data_root() -> Option<PathBuf> {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".save-data");
        p.is_dir().then_some(p)
    }

    #[test]
    fn reads_real_save() {
        let Some(root) = save_data_root() else {
            eprintln!("skipping: no .save-data directory");
            return;
        };

        let dirs = find_save_dirs(&root).unwrap();
        assert!(
            !dirs.is_empty(),
            "no Demon's Souls save found under {}",
            root.display()
        );

        for dir in &dirs {
            println!("{} ({})", dir.directory_name(), dir.path.display());
            for slot in 1..=SLOT_COUNT {
                let Some(file) = dir.active_slot_file(slot) else {
                    println!("  slot {slot}: no file");
                    continue;
                };
                let bytes = std::fs::read(file).unwrap();
                match Character::read(&bytes) {
                    Ok(c) => {
                        println!("  slot {slot}: {} -> {c:#?}", file.display());

                        // A no-op write must leave the file byte-for-byte identical.
                        let mut copy = bytes.clone();
                        c.write_into(&mut copy).unwrap();
                        assert!(copy == bytes, "no-op write changed {}", file.display());
                    }
                    Err(e) => println!("  slot {slot}: {} -> {e}", file.display()),
                }
            }
        }
    }
}
