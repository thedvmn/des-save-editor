#!/usr/bin/env bash
# Builds dist/des-save-editor-<version>-x86_64.AppImage.
# Run from the repository root. Downloads appimagetool on first use.
set -euo pipefail

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
APPDIR=target/appimage/AppDir
TOOL=target/appimage/appimagetool-x86_64.AppImage
TOOL_URL=https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage

cargo build --release

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps"
cp target/release/des-save-editor "$APPDIR/usr/bin/"
cp packaging/linux/des-save-editor.desktop "$APPDIR/usr/share/applications/"
cp packaging/linux/des-save-editor.desktop "$APPDIR/"
cp assets/icons/icon-256.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/des-save-editor.png"
cp assets/icons/icon-256.png "$APPDIR/des-save-editor.png"
ln -s usr/bin/des-save-editor "$APPDIR/AppRun"

if [ ! -x "$TOOL" ]; then
    curl -fL -o "$TOOL" "$TOOL_URL"
    chmod +x "$TOOL"
fi

mkdir -p dist
# --appimage-extract-and-run avoids needing FUSE (e.g. in containers and CI).
ARCH=x86_64 "$TOOL" --appimage-extract-and-run "$APPDIR" "dist/des-save-editor-$VERSION-x86_64.AppImage"
echo "Built dist/des-save-editor-$VERSION-x86_64.AppImage"
