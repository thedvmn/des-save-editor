//! Locating Demon's Souls save directories and the active file for each slot.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::sfo::Sfo;

/// Known Demon's Souls title IDs (US, EU, JP, JP "the Best", Asia).
const TITLE_IDS: [&str; 5] = [
    "BLUS30443",
    "BLES00932",
    "BCJS30022",
    "BCJS70013",
    "BCAS20071",
];

pub const SLOT_COUNT: u8 = 4;

/// A save directory that contains a `PARAM.SFO` belonging to Demon's Souls.
#[derive(Debug, Clone)]
pub struct SaveDir {
    pub path: PathBuf,
    pub sfo: Sfo,
    /// Upper-cased file name -> actual path, so lookups are case-insensitive.
    files: HashMap<String, PathBuf>,
}

impl SaveDir {
    /// Opens `path` as a save directory if it has a Demon's Souls `PARAM.SFO`.
    pub fn open(path: &Path) -> io::Result<Option<Self>> {
        let mut files = HashMap::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let name = entry.file_name().to_string_lossy().to_uppercase();
                files.insert(name, entry.path());
            }
        }

        let Some(sfo_path) = files.get("PARAM.SFO") else {
            return Ok(None);
        };
        let sfo = match Sfo::parse(&fs::read(sfo_path)?) {
            Ok(sfo) => sfo,
            Err(_) => return Ok(None),
        };
        if !is_demons_souls(&sfo) {
            return Ok(None);
        }

        Ok(Some(Self {
            path: path.to_owned(),
            sfo,
            files,
        }))
    }

    /// The `SAVEDATA_DIRECTORY` name from the SFO, e.g. `BLES00932DEMONSS005`.
    pub fn directory_name(&self) -> &str {
        self.sfo.get_str("SAVEDATA_DIRECTORY").unwrap_or_default()
    }

    /// The file the game currently treats as authoritative for `slot` (1-based).
    pub fn active_slot_file(&self, slot: u8) -> Option<&Path> {
        self.resolve_rotation(&slot_variants(slot))
    }

    /// The game rotates each save between three file names [A -> B -> C -> A].
    /// The active one is the variant whose successor is missing.
    fn resolve_rotation(&self, variants: &[String; 3]) -> Option<&Path> {
        for i in 0..3 {
            let next = &variants[(i + 1) % 3];
            if !self.files.contains_key(next) {
                if let Some(p) = self.files.get(&variants[i]) {
                    return Some(p);
                }
            }
        }
        // All three exist: fall back to the first; a stale file is caught by
        // the USER.DAT sanity check.
        variants
            .iter()
            .find_map(|v| self.files.get(v))
            .map(PathBuf::as_path)
    }
}

/// Slot 1 is USER/1USER/2USER.DAT; slot N is 0(N-1)/10(N-1)/20(N-1)USER.DAT.
fn slot_variants(slot: u8) -> [String; 3] {
    if slot == 1 {
        ["USER.DAT", "1USER.DAT", "2USER.DAT"].map(String::from)
    } else {
        let s = slot - 1;
        [
            format!("0{s}USER.DAT"),
            format!("10{s}USER.DAT"),
            format!("20{s}USER.DAT"),
        ]
    }
}

fn is_demons_souls(sfo: &Sfo) -> bool {
    let dir = sfo.get_str("SAVEDATA_DIRECTORY").unwrap_or_default();
    TITLE_IDS.iter().any(|id| dir.starts_with(id))
        || sfo
            .get_str("TITLE")
            .is_some_and(|t| t.contains("Demon's Souls"))
}

/// Finds Demon's Souls save directories at `root`.
///
/// `root` may be a save directory itself, or a parent such as RPCS3's
/// `dev_hdd0/home/00000001/savedata` folder, whose children are scanned.
pub fn find_save_dirs(root: &Path) -> io::Result<Vec<SaveDir>> {
    if let Some(dir) = SaveDir::open(root)? {
        return Ok(vec![dir]);
    }

    let mut found = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            // Unreadable sub-directories are skipped rather than failing the scan.
            if let Ok(Some(dir)) = SaveDir::open(&entry.path()) {
                found.push(dir);
            }
        }
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_variant_names() {
        assert_eq!(slot_variants(1), ["USER.DAT", "1USER.DAT", "2USER.DAT"]);
        assert_eq!(
            slot_variants(3),
            ["02USER.DAT", "102USER.DAT", "202USER.DAT"]
        );
    }
}
