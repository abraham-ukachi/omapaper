# Changelog

All notable changes to this project will be documented in this file. Versions follow [SemVer](https://semver.org), see [CONTRIBUTING.md](./CONTRIBUTING.md#git-commit-messages) for the commit guidelines.

## [0.1.0] - 2026-11-15

### Added
- :sparkles: **OmaPaper 0.1.0** — a 100% Rust, Omarchy-native design canvas: daemon, MCP (stdio + HTTP), canvases, pages, frames, streaming, design tokens, comments, presence, PNG/PDF export, all 23 Omarchy themes plus 2 ab-nextjs schemes, a Slint cockpit and a GTK3/WebKit canvas window.
- :robot: **45 MCP tools** mirroring the Doop + Paper union (canvas, pages, frames, node tools, tokens, comments, guides, decisions).
- :art: Every Omarchy theme, painted live from `~/.config/omarchy/current/theme`, with a Tokyo Night fallback.
- :package: User-level installer (`scripts/install.sh`), desktop entry and a `PKGBUILD` for Arch/Omarchy.
- :memo: README, AGENTS, CONTRIBUTING, MCP reference and CI workflow.
- :framed_picture: Logo and launcher icons in `packaging/icons`.

### Notes
- Local only: the daemon binds `127.0.0.1:4310` by default. No account, no cloud, no subscription.
- Screenshots and PDF export shell out to headless Chromium (`OMAPPAPER_CHROMIUM` to override).
