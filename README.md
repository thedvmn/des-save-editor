# Demon's Souls Save Editor

A save editor for the PS3 version of Demon's Souls, made for saves from the [RPCS3](https://rpcs3.net) emulator.

I made this because I couldn't find a save editor that ran on Linux. It runs on Linux and Windows.

## Features

- **Levels:** change each stat (1–99). Levels purchased updates automatically.
- **Inventory:** edit souls, item counts and durability. Add or remove items.
- **Thomas:** the same editing for items stored with Stockpile Thomas.
- **World Tendency:** set character tendency and each world's tendency (−200 black to +200 white).

Changes are only written when you press **Save** and confirm. The editor won't overwrite a save that changed on disk since it was opened. Equipped items can't be removed.

## Backups

When you open a save folder, the editor offers to back it up. Pick a folder and a timestamped copy of the save goes there. Tick **Don't show again** to skip the prompt. You can make a backup at any time with **Create Backup…**.

Saving never makes a backup on its own.

## Download

Get the latest build from the Releases page:

- **Linux:** `des-save-editor-<version>-x86_64.AppImage`. Make it executable and run it.
- **Windows:** `des-save-editor-windows-x86_64.exe`

## Usage

1. Close the game in RPCS3.
2. Click **Open folder…** and pick either a single save folder or RPCS3's `savedata` folder:
   - Linux: `~/.config/rpcs3/dev_hdd0/home/00000001/savedata/`
   - Linux (Flatpak): `~/.var/app/net.rpcs3.RPCS3/config/rpcs3/dev_hdd0/home/00000001/savedata/`
   - Windows: `<RPCS3 folder>\dev_hdd0\home\00000001\savedata\`
3. Pick a character, make your changes, then click **Save…**

Demon's Souls save folders start with the game's ID, for example `BLES00932` (EU) or `BLUS30443` (US). So far it has only been tested with an EU save.

## Building

You need a recent stable Rust toolchain.

```sh
cargo run --release                    # run
cargo test                             # tests
./packaging/linux/build-appimage.sh    # Linux AppImage, written to dist/
```

On Windows, `cargo build --release` produces `target/release/des-save-editor.exe`.

## Credits

The save format and item data come from these projects:

- [jij3x/DemonSave-PS3](https://github.com/jij3x/DemonSave-PS3): item database (MIT, see `data/LICENSE-des-db`) and format notes.
- [Wulf2k/DeS-SaveEdit](https://github.com/Wulf2k/DeS-SaveEdit): the original Demon's Souls save editor.

The icon is original artwork made for this project.

## License

MIT. See `LICENSE`.

## AI disclaimer

This tool was made in a couple of hours with AI assistance. Back up your saves, and please report any problems you find.