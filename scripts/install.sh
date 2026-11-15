#!/usr/bin/env bash
# Installs OmaPaper for the current user: binaries in ~/.local/bin, the icon
# and desktop entry in ~/.local/share. No sudo, no package manager.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
prefix="${PREFIX:-$HOME/.local}"
data="${XDG_DATA_HOME:-$HOME/.local/share}"

say() { printf '\033[1;34m::\033[0m %s\n' "$*"; }

say "building OmaPaper (release)"
cargo build --release --locked --workspace --manifest-path "$root/Cargo.toml"

say "installing binaries into $prefix/bin"
install -Dm755 "$root/target/release/omapaperd" "$prefix/bin/omapaperd"
install -Dm755 "$root/target/release/omapaper-mcp" "$prefix/bin/omapaper-mcp"
install -Dm755 "$root/target/release/omapaper" "$prefix/bin/omapaper"
install -Dm755 "$root/target/release/omapaper-canvas" "$prefix/bin/omapaper-canvas"

say "installing icons into $data/icons/hicolor"
for size in 16 24 32 48 64 128 256 512; do
  install -Dm644 "$root/packaging/icons/hicolor/${size}x${size}/apps/omapaper.png" \
    "$data/icons/hicolor/${size}x${size}/apps/omapaper.png"
done
install -Dm644 "$root/packaging/icons/omapaper.svg" \
  "$data/icons/hicolor/scalable/apps/omapaper.svg"

say "installing the desktop entry"
install -Dm644 "$root/packaging/omapaper.desktop" "$data/applications/omapaper.desktop"

command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$data/icons/hicolor" || true
command -v update-desktop-database >/dev/null && update-desktop-database -q "$data/applications" || true

say "done — launch OmaPaper from your app launcher, or run: omapaper"
case ":$PATH:" in
  *":$prefix/bin:"*) ;;
  *) say "note: add $prefix/bin to PATH to run it from a terminal" ;;
esac
