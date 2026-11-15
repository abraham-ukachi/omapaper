# AGENTS.md — OmaPaper 🤖

> Read this first if you are an AI agent (or a new human) picking up this repo.

**OmaPaper** is a 100% Rust design canvas for humans and AI agents, with Omarchy's look & feel.
The long-term goal: **one local canvas where humans and agents design together**, with no cloud, no account and no subscription.

## Golden rules

1. 🦀 **Core first.** Model, theme, store, tokens, HTML and render logic lives in `crates/omapaper-core` with unit tests. The daemon, MCP and app crates only wire to core.
2. 🧪 `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` at 0 before every push.
3. 🎨 **Omacalc is the design reference** (see CONTRIBUTING.md › Design rules). Don't add colors; mix the theme's two.
4. 🗄️ **SQLite is the only store.** No Postgres, no Bun, no cloud. One file under `~/.local/share/omapaper/`.
5. 🔐 **Local only.** The daemon binds `127.0.0.1` by default; never expose it to the network without an explicit `--host`.
6. 📄 **Frames are whole HTML documents.** Keep `data-op-id`, `data-omapaper-on` and `data-omapaper-action` attributes intact so node tools, comments and prototypes keep working.
7. 🔖 **Bump before you tag.** Set `[workspace.package] version` in `Cargo.toml` (and `Cargo.lock`), commit, then push the `vX.Y.Z` tag.
8. ✍️ Gitmoji commits, a CHANGELOG entry per version.

## Where things are

| Task | Files |
|:-----|:------|
| Add a model type / canvas shape | `crates/omapaper-core/src/model.rs` |
| Add or change a theme | `crates/omapaper-core/src/theme.rs` |
| Change storage / queries | `crates/omapaper-core/src/store.rs` |
| Change token logic | `crates/omapaper-core/src/tokens.rs` |
| Change HTML parsing / selectors | `crates/omapaper-core/src/html.rs` |
| Change rendering / screenshots | `crates/omapaper-core/src/render.rs` |
| Change agent guidance text | `crates/omapaper-core/src/guide.rs` |
| Add or change an MCP tool | `crates/omapaper-mcp/src/tools.rs`, `lib.rs` |
| Change the daemon / REST / WS | `crates/omapaperd/src/{main,api,live}.rs` |
| Change the canvas UI | `canvas/{index.html,app.css,app.js}` |
| Change the cockpit (Slint) | `app/ui/app.slint`, `app/src/main.rs` |
| Change the WebKit viewer | `canvas-viewer/src/main.rs` |
| Packaging / icons | `packaging/`, `scripts/install.sh` |

## Binaries

`omapaper` (cockpit), `omapaperd` (daemon), `omapaper-mcp` (stdio MCP), `omapaper-canvas` (WebKit viewer).
