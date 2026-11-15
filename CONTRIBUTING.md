# Contributing to OmaPaper 🤝

Thanks for helping! OmaPaper is small on purpose: one canvas, many agents, Omarchy's look.

## Getting started 🚀

```sh
git clone https://github.com/abraham-ukachi/omapaper.git && cd omapaper
cargo test --workspace              # core, mcp, daemon tests
cargo run -p omapaperd             # the daemon on 127.0.0.1:4310
cargo run -p omapaper              # the cockpit
```

Arch / Omarchy packaging:

```sh
cd packaging && makepkg -si
```

## Project layout 🗂️

| No. | Path | What lives there |
|:----|:-----|:-----------------|
| 1 | `crates/omapaper-core` | Model, theme, store, tokens, HTML, render, guide. No UI, fully tested |
| 2 | `crates/omapaper-mcp` | 45 MCP tools, JSON-RPC over stdio + HTTP |
| 3 | `crates/omapaperd` | Daemon: REST, WebSocket, embedded canvas, MCP HTTP |
| 4 | `app` | Slint cockpit |
| 5 | `canvas-viewer` | GTK3 + WebKit2GTK canvas window |
| 6 | `canvas` | Hand-written HTML/CSS/JS canvas UI (no build step) |
| 7 | `packaging` | Desktop entry, PKGBUILD, icons |
| 8 | `docs` | MCP reference and design notes |

## Design rules 🎨

1. Follow **Omacalc**: page color + ink color, everything else is a mix of the two (5% keys, 13% borders, 16% operators & dividers, 50% muted). The primary key is inverted ink.
2. iA Writer Mono everywhere. No icons with fills, 1.6 px rounded strokes only.
3. The accent color is a whisper: the selected theme and the focus ring, nothing else.
4. Themes come from Omarchy (`~/.config/omarchy/current/theme`); never hard-code a palette. Fall back to Tokyo Night.

## Git commit messages ✍️

We use [gitmoji](https://gitmoji.dev) + a short sentence in the imperative mood:

| Emoji | Code | When |
|:------|:-----|:-----|
| ✨ | `:sparkles:` | a new feature |
| 🐛 | `:bug:` | a bug fix |
| 💄 | `:lipstick:` | UI & style |
| 📝 | `:memo:` | docs |
| 🔧 | `:wrench:` | config & tooling |
| ✅ | `:white_check_mark:` | tests |
| 👷 | `:construction_worker:` | CI |
| 🔖 | `:bookmark:` | a release (`Release 0.1.0`) |

Example: `:sparkles: Add the WebKit canvas viewer`
