//! Backups and safe writes.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Copies every file in `save_dir` into a new folder inside `root`, named
/// after the save folder plus a timestamp, and returns its path.
pub fn backup_save_dir(save_dir: &Path, root: &Path) -> io::Result<PathBuf> {
    let name = save_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "save".to_owned());
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");

    let mut dest = root.join(format!("{name}_{stamp}"));
    let mut n = 2;
    while dest.exists() {
        dest = root.join(format!("{name}_{stamp}-{n}"));
        n += 1;
    }
    fs::create_dir_all(&dest)?;

    for entry in fs::read_dir(save_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let target = dest.join(entry.file_name());
        let copied = fs::copy(entry.path(), &target)?;
        if copied != entry.metadata()?.len() {
            return Err(io::Error::other(format!(
                "backup of {} is incomplete",
                entry.path().display()
            )));
        }
    }
    Ok(dest)
}

/// Writes `bytes` to a temporary file next to `path`, then renames it over
/// `path`, so a failed write never leaves a half-written save behind.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut tmp_name = path.file_name().unwrap_or_default().to_owned();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);

    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_replaces_contents() {
        let dir = std::env::temp_dir().join(format!("des-save-editor-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("USER.DAT");
        fs::write(&file, b"old").unwrap();

        write_atomic(&file, b"new contents").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"new contents");
        assert!(!dir.join("USER.DAT.tmp").exists());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn backup_copies_every_file() {
        let tmp =
            std::env::temp_dir().join(format!("des-save-editor-backup-{}", std::process::id()));
        let save = tmp.join("BLES00932DEMONSS005");
        fs::create_dir_all(&save).unwrap();
        fs::write(save.join("USER.DAT"), b"user").unwrap();
        fs::write(save.join("PARAM.SFO"), b"sfo").unwrap();

        let first = backup_save_dir(&save, &tmp.join("backups")).unwrap();
        let second = backup_save_dir(&save, &tmp.join("backups")).unwrap();
        assert_ne!(first, second, "a second backup never overwrites the first");
        assert!(
            first
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("BLES00932DEMONSS005_")
        );
        assert_eq!(fs::read(first.join("USER.DAT")).unwrap(), b"user");
        assert_eq!(fs::read(first.join("PARAM.SFO")).unwrap(), b"sfo");

        fs::remove_dir_all(&tmp).unwrap();
    }
}
