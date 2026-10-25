//! User preferences, stored as `key=value` lines in
//! `~/.config/des-save-editor/settings.txt` (Linux) or
//! `%APPDATA%\des-save-editor\settings.txt` (Windows).

use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Ask to make a backup whenever a save folder is loaded.
    pub backup_prompt: bool,
    /// Where the last backup went; the folder picker starts there.
    pub last_backup_dir: Option<PathBuf>,
    /// File the settings are saved to; `None` keeps them in memory only.
    path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            backup_prompt: true,
            last_backup_dir: None,
            path: None,
        }
    }
}

impl Settings {
    /// Loads the user's settings, falling back to defaults if there are none.
    pub fn load() -> Self {
        let path = dirs::config_dir().map(|d| d.join("des-save-editor").join("settings.txt"));
        let mut settings = path
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .map(|text| Self::parse(&text))
            .unwrap_or_default();
        settings.path = path;
        settings
    }

    fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for (key, value) in text.lines().filter_map(|l| l.split_once('=')) {
            match key.trim() {
                "backup_prompt" => s.backup_prompt = value.trim() != "false",
                "last_backup_dir" if !value.trim().is_empty() => {
                    s.last_backup_dir = Some(PathBuf::from(value.trim()));
                }
                _ => {}
            }
        }
        s
    }

    fn serialize(&self) -> String {
        let mut text = format!("backup_prompt={}\n", self.backup_prompt);
        if let Some(dir) = &self.last_backup_dir {
            text += &format!("last_backup_dir={}\n", dir.display());
        }
        text
    }

    pub fn save(&self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = Settings {
            backup_prompt: false,
            last_backup_dir: Some(PathBuf::from("/home/me/DeS backups")),
            path: None,
        };
        assert_eq!(Settings::parse(&s.serialize()), s);
        assert_eq!(Settings::parse(""), Settings::default());
    }
}
