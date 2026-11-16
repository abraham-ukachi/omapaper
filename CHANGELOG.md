# Changelog

All notable changes to this project will be documented in this file. Versions follow [SemVer](https://semver.org), see [CONTRIBUTING.md](./CONTRIBUTING.md#git-commit-messages) for the commit guidelines.

## [0.8.0-beta.2] - 2026-11-16

### Fixed
- :bug: Fixed live image route registration (`/i/{filename}`).

## [0.8.0-beta.1] - 2026-11-16

### Added
- :framed_picture: Phase C: public live frame image URLs at `/i/{frameId}.png`.

### Notes
- Beta release. Website/GitHub/live-app imports and React/Tailwind export remain planned.

## [0.7.0-beta.1] - 2026-11-16

### Added
- :white_check_mark: Phase B: approval lifecycle now moves approved screens to needs review after edits.
- :arrow_forward: Shareable prototype route via `?proto=1`.

### Notes
- Beta release. Prototype connectors and full interaction matrix remain planned.

## [0.6.0-beta.2] - 2026-11-16

### Added
- :art: Resize handles for inserted shapes.
- :pencil2: Double-click text layers for inline editing.
- :white_check_mark: Selection outline and layer handles.

### Notes
- Completes Phase A foundation scope; pen paths, rulers/guides and full layer tree remain planned.

## [0.6.0-beta.1] - 2026-11-16

### Added
- :art: Phase A canvas editor tools: select, hand, rectangle, ellipse, text.
- :art: Click-to-insert shapes/text into a frame with positioned layers.
- :art: Layer list in the inspector with selection and deletion.
- :mag: Zoom presets (50/100/200%) and pixel grid at high zoom.
- :see_no_evil: Hide-all-UI toggle.

### Notes
- Beta release. Phase A foundation; pen paths, rich text, rulers/guides and full layer tree remain in progress.

## [0.5.0-beta.1] - 2026-11-16

### Added
- :package: `/api/version` endpoint exposing app and stable API versions.
- :electric_plug: Plugin manifest foundation: scans `$DATA_DIR/plugins/*.json` and exposes `/api/plugins`.
- :gear: Tagged-release CI workflow and updated packaging metadata.

### Notes
- Beta release. Plugin code execution is not enabled yet.

## [0.4.1] - 2026-11-16

### Added
- :iphone: Touch-friendly node selection inside frames (no click delay).
- :iphone: Mobile UX polish: touch-action, overscroll containment, and font sizing to avoid input zoom.

### Notes
- Completes the v0.4 mobile editing pass.

## [0.4.0-beta.3] - 2026-11-16

### Added
- :iphone: Touch support for frame resizing, selection drawer auto-close, and pinch-to-zoom on the canvas stage.
- :iphone: Larger touch targets for resize handles and controls on narrow screens.

### Notes
- Beta release. Mobile editing is improving but remains best on larger screens.

## [0.4.0-beta.2] - 2026-11-16

### Fixed
- :bug: Canvas viewer now loads after the window is realized, fixing first-open blank canvas.
- :bug: Daemon readiness wait increased for cold starts.

### Notes
- Beta release.

## [0.4.0-beta.1] - 2026-11-16

### Added
- :globe_with_meridians: Cross-platform data directories (Windows/macOS/Linux) and URL opening.
- :iphone: Mobile-friendly canvas viewer with drawer navigation for pages/frames/inspector.
- :white_check_mark: CI build matrix across Ubuntu, macOS and Windows.

### Notes
- Beta release. Mobile editing remains desktop-first.

## [0.3.2-beta.1] - 2026-11-15

### Added
- :sparkles: Duplicate canvas action in the canvas UI.
- :sparkles: `/api/canvases/{id}/duplicate` endpoint copying pages, tokens, frames and comments.

### Notes
- Beta release.

## [0.3.1-beta.1] - 2026-11-15

### Added
- :sparkles: Frame filter in the canvas rail.
- :sparkles: Marked v0.3 team sharing complete.

### Notes
- Beta release.

## [0.3.0-beta.5] - 2026-11-15

### Added
- :sparkles: Page management in the canvas: create, rename and delete pages.
- :sparkles: Frames move to another page when their page is deleted.

### Notes
- Beta release.

## [0.3.0-beta.4] - 2026-11-15

### Added
- :sparkles: LAN-aware Share button using `/api/network` to copy a reachable LAN URL.
- :sparkles: HTML file import in the canvas UI (Import button).
- :sparkles: `/api/network` endpoint exposing local addresses and port.

### Notes
- Beta release.

## [0.3.0-beta.3] - 2026-11-15

### Added
- :sparkles: Launcher closes after opening a canvas; theme now follows Omarchy live.
- :sparkles: Launcher count moved left of "Canvases"; refresh/delete are icon buttons.
- :sparkles: Removed theme chip from launcher; added safe canvas delete.
- :sparkles: Dashboard tab in the canvas with overview, quick actions, frames and comments.

### Notes
- Beta release.

## [0.3.0-beta.2] - 2026-11-15

### Added
- :sparkles: Share button now copies a view-only link (`?canvas=...&readonly=1`).
- :sparkles: Canvas selection respects `?canvas=` in the URL for shared links.

### Notes
- Beta release.

## [0.3.0-beta.1] - 2026-11-15

### Added
- :sparkles: Read-only viewer mode via `?readonly=1`, hiding editing controls and the resize handle.
- :sparkles: `OMAPPAPER_HOST` env support and a Share button that copies the current URL for LAN sharing.
- :sparkles: Marked v0.2 complete (resize handles, multi-select, undo/redo, HTML import).

### Notes
- Beta release: core features are still evolving and the app may be buggy.

## [0.1.2] - 2026-11-15

### Added
- :sparkles: Shift-click multi-select of nodes in the canvas with a selection count in the inspector.
- :sparkles: `nodeIds` parameter on node tools to edit several nodes in one call.
- :sparkles: `import_html` MCP tool to create a frame from raw HTML or a local HTML file.

### Notes
- Patch release on top of 0.1.1; no breaking API changes.

## [0.1.1] - 2026-11-15

### Added
- :sparkles: Drag-to-resize handles on canvas frames, persisted through the frame API.
- :sparkles: Per-frame undo/redo stacks with inspector buttons and Ctrl+Z / Ctrl+Shift+Z shortcuts.
- :sparkles: `all` flag on `update_styles`, `set_text_content`, `rename_nodes` and `delete_nodes` to edit every matching element at once.

### Notes
- Patch release on top of 0.1.0; no breaking API changes.

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
