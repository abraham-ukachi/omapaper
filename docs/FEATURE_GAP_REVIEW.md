# Feature Gap Review: Doop + Paper Desktop vs. OmaPaper

Date: 2026-11-16  
Scope: product features only; billing, accounts, cloud, workspaces and seat math are intentionally excluded.

## 1. Method

This review combines:

- Doop public docs and README (`doop.design`, GitHub `kgoedecke/doop`)
- Paper public site/docs/build log (`paper.design`)
- OmaPaper README and current canvas implementation (`canvas/app.js`, `canvas/app.css`, crates)

The goal is not to clone either app. OmaPaper deliberately keeps a local-first, account-free, SQLite-backed, Omarchy-themed model. The goal is to identify missing capabilities that make Doop/Paper powerful for human + agent design work, then sequence them without reintroducing cloud/accounts/billing.

## 2. Current OmaPaper baseline

OmaPaper already has a strong local core:

- Local daemon on `127.0.0.1:4310`
- MCP stdio + HTTP, WebSocket presence/live edits
- Canvases, pages, frames
- Frames as complete HTML documents
- Chunked frame HTML streaming (`append_frame_html`)
- Design tokens typed per theme, CSS/Tailwind export
- Threaded comments anchored to elements or canvas points
- Presence for humans/tabs/agents
- PNG/PDF/HTML export
- 23 Omarchy themes + live theme
- Desktop cockpit
- Mobile touch basics (pointer events, pinch zoom, drawer auto-close)
- Node selection/multi-select, inspector (tag/attrs/styles/children/computed/JSX)
- Team sharing over LAN read-only viewers
- Plugin manifests and execution, stable API version

OmaPaper does **not** yet have most of the drawing/editing/tooling depth found in Doop/Paper.

## 3. Doop feature inventory (non-billing)

### Canvas model
- Pages within a canvas: add, rename, reorder, delete, move frames between pages
- Page background color per page
- Undo/redo for page edits
- Frames as real HTML in sandboxed iframes
- Frame resize, move, snap to neighbors/guides
- Frame approval lifecycle: draft, ready_for_dev, needs_review
- Approval attribution and revision checks
- Prototype interactions and prototype player

### Editor tools
- Select, Hand/Pan, Frame, Rectangle, Circle/Ellipse, Vector pen, Text, Comment
- Marquee selection, shift/cmd multi-select
- Shapes drawn inside frames become positioned HTML/SVG layers
- Rich text editing with formatting toolbar, lists, run-level styling
- Path editing: anchors, Bézier handles, corner/curve conversion
- Layer tree: search, reorder, reparent, hide, lock, collapse, context actions
- Rulers and guides (page-level, snap within 8px)
- Zoom 8%–1600%, wheel pan, pinch/ctrl zoom, pixel grid from 300%
- Resizable panels, hide-all-UI mode
- Light/Dark/System themes
- Mobile canvas: one-finger pan, pinch zoom, tap comment, bottom sheets

### Collaboration and presence
- Live cursors, presence avatars
- Follow mode (click avatar to follow viewport)
- Per-frame editing indicators
- Activity feed
- Threaded comments, @mentions, email digests
- View/comment/edit link levels
- Canvas access rules shared by REST/MCP

### Agent platform
- MCP server with OAuth and agent keys
- Built-in Doop Agent with specialist roles
- Board cards and task queue
- @mentions in comments trigger agent work
- Agent status, tasks, presence
- Taste/design memory: pinned exemplars, decisions, distiller style rules
- Automations: scheduled and approval-triggered workflows

### Import/export/content
- Website import: single page or whole site discovery
- Live app sync snippet
- GitHub repo import and design system extraction
- Figma/Canva/PSD import/export in cloud (excluded from OSS)
- PNG/JPG/vector PDF export
- Public live image URLs (`/i/<frame>.png`)
- Frame export and page-bound PDF export

### Integrations and webhooks
- Linear, Slack, Meta Ads integrations
- Outbound webhooks for comments, frames, agent completion/failure
- Element links with CSS selectors

## 4. Paper feature inventory (non-billing)

### Canvas model
- HTML/CSS canvas, real flexbox
- Frames/artboards and layers
- Constraints panel for layout during parent resize
- Backdrop filters
- OpenType font features
- Text details panel
- File dashboard with search/view options

### Editor tools
- Move, Pan, Frame, Rectangle, Text, Pen, Shaders, Eyedropper
- Wrap selection in frame/flex layout
- Pixel grid at 600%+
- Hide/minimize UI
- Keyboard-driven creation and styling
- Copy as React/Tailwind
- Frame screenshot
- Crop image rather than scale
- Paste to replace

### Content and data
- Paste from Figma, HTML, real sites
- Snapshot extension for editable site capture
- Real data from files/CMS/database
- Prompt agents to pull data from tools/APIs
- Update copy back into codebase
- Bidirectional code sync: tokens, styles, components
- Image generation and SVG generation
- Shaders menu

### Collaboration
- Real-time multiplayer
- Free editor/viewer seats (billing excluded)
- Comments for agents

### Agent platform
- Desktop app runs MCP server
- Connect IDE/CLI agents
- Design tokens sync between codebase and canvas
- Agent reads/writes HTML/CSS

### Export
- Tailwind classes
- React with CSS
- Agent-built sites

## 5. Gap matrix

Legend: ✅ present, 🟡 partial, ❌ missing

| Capability | Doop | Paper | OmaPaper |
|---|---:|---:|---:|
| Local-first no account | ✅ (self-host) | ❌ (account) | ✅ |
| SQLite single-file storage | ❌ (Postgres) | ❌ | ✅ |
| Omarchy themes | ❌ | ❌ | ✅ |
| Canvases/pages/frames | ✅ | ✅ | ✅ |
| Frame HTML streaming | ✅ | ❌ | ✅ |
| Design tokens | ✅ | ✅ | ✅ |
| Comments | ✅ | ✅ | ✅ |
| Presence/live cursors | ✅ | ✅ | ✅ |
| MCP agents | ✅ | ✅ | ✅ |
| Mobile touch basics | ✅ | ❌ | 🟡 |
| Page background per page | ✅ | ❌ | ❌ |
| Frame approval lifecycle | ✅ | ❌ | 🟡 (approval field exists, no full lifecycle) |
| Clickable prototypes | ✅ | ❌ | 🟡 (attributes exist, player incomplete) |
| Shape tools (rect/circle/ellipse) | ✅ | ✅ | ❌ |
| Vector pen/path editing | ✅ | ✅ | ❌ |
| Text tool/rich text editing | ✅ | ✅ | ❌ |
| Layer tree with reorder/reparent/hide/lock | ✅ | ✅ | ❌/🟡 |
| Rulers/guides/snapping | ✅ | 🟡 | ❌ |
| Zoom presets/pixel grid | ✅ | ✅ | 🟡 |
| Hide-all-UI mode | ✅ | ✅ | ❌ |
| Rich text runs | ✅ | ✅ | ❌ |
| Constraints/flex layout editor | 🟡 | ✅ | 🟡 (inspector styles only) |
| Backdrop filters | ❌ | ✅ | ❌ |
| OpenType features | ❌ | ✅ | ❌ |
| Shaders/effects menu | ❌ | ✅ | ❌ |
| Eyedropper | ❌ | ✅ | ❌ |
| Image generation | ✅ | ✅ | ❌ |
| SVG generation | ❌ | ✅ | ❌ |
| Copy as React/Tailwind | ❌ | ✅ | 🟡 (HTML/JSX only) |
| Frame screenshot command | ❌ | ✅ | ✅ (API) |
| Crop image | ❌ | ✅ | ❌ |
| Paste from Figma/HTML/site | 🟡 | ✅ | 🟡 (HTML import) |
| Website import (single/whole site) | ✅ | 🟡 | ❌ |
| GitHub repo import/design system extraction | ✅ | ❌ | ❌ |
| Live app sync snippet | ✅ | 🟡 | ❌ |
| Real data from DB/CMS/files | ❌ | ✅ | ❌ |
| Bidirectional code sync | ❌ | ✅ | ❌ |
| Taste/design memory/distiller | ✅ | ❌ | ❌ |
| Built-in agent roles/board cards | ✅ | ❌ | ❌ |
| Automations/scheduled tasks | ✅ | ❌ | ❌ |
| Outbound webhooks | ✅ | ❌ | ❌ |
| Integrations (Linear/Slack/Meta) | ✅ | ❌ | ❌ |
| Public live image URLs | ✅ | ❌ | ❌ |
| File dashboard/search/view modes | ❌ | ✅ | ❌ |
| Desktop app tabs/native menus/updates | ✅ | ✅ | 🟡 |
| doop:// deep links/OAuth handoff | ✅ | ❌ | ❌ |
| Plugin system | ❌ | ❌ | 🟡 (manifests only) |
| Stable API versioning | 🟡 | ❌ | ✅ (new) |

## 6. Implementation phases

### Phase A — Canvas editor parity (highest visual/tooling value)

Goal: make OmaPaper a real drawing/editing canvas, not just an HTML inspector.

Steps:
1. Add tool state and toolbar: Select, Hand, Frame, Rectangle, Ellipse, Text, Pen, Comment.
2. Implement shape creation in the WebKit canvas by injecting a lightweight overlay editor.
3. Convert drawn shapes into positioned HTML/SVG layers inside the selected frame.
4. Add resize/move handles with aspect ratio, guides, snapping.
5. Add text creation and inline rich text editing.
6. Add vector pen with anchor/handle editing.
7. Add layer tree panel with search, reorder, reparent, hide, lock.
8. Add rulers/guides and zoom presets/pixel grid.
9. Add hide-all-UI mode and panel resizing.

### Phase B — Prototypes and approvals

Goal: complete Doop-style interaction and dev-handoff features.

Steps:
1. Finalize prototype attribute schema (`data-op-on`, `data-op-action`, transition).
2. Add Prototype tab in inspector and canvas connectors.
3. Implement Play mode full-screen player with back/forward.
4. Add shareable `/proto` route.
5. Implement approval lifecycle and revision checks.
6. Add approval badges and filters in rail.
7. Add MCP tools for prototype get/set and approval.

### Phase C — Content import and export

Goal: bring real material onto the canvas and export in more formats.

Steps:
1. Website import: capture one page as editable frame.
2. Multi-page site discovery/import.
3. Live app sync snippet.
4. GitHub repo import + design system extraction.
5. Paste from HTML/Figma/PSD where feasible.
6. Export React/Tailwind, public live image URLs.
7. Image crop and asset management.

### Phase D — Collaboration and presence depth

Goal: improve multiplayer editing.

Steps:
1. Per-frame editing indicators.
2. Follow mode.
3. Activity feed.
4. Element links with selectors.
5. View/comment/edit link levels.
6. Email digests (optional, local SMTP config).
7. Mobile bottom sheets and full touch editing.

### Phase E — Agent platform depth

Goal: move from MCP tools to agent-first workflow.

Steps:
1. Built-in agent runner (optional, user-provided API keys).
2. Specialist roles and board cards.
3. @mention-triggered tasks.
4. Task feedback and status history.
5. Taste/design memory and distiller.
6. Automations (scheduled and approval-triggered).
7. Outbound webhooks.

### Phase F — Data and code sync

Goal: Paper-style bidirectional workflow.

Steps:
1. Connect local files/repos as data sources.
2. Pull real data into frames.
3. Push copy/content updates back to codebase.
4. Sync design tokens/components between codebase and canvas.
5. Image and SVG generation hooks.

### Phase G — Packaging and desktop polish

Goal: make install/run experience match desktop apps.

Steps:
1. Desktop app tabs, native menus, check-for-updates.
2. Deep links (`omapaper://`) for auth/integration handoff.
3. macOS/Windows/Linux packaging.
4. File dashboard with search/view modes.
5. Plugin execution beyond manifests.

## 7. Status

- **v0.6** — canvas editor parity: shipped.
- **v0.7** — prototypes and approvals: shipped.
- **v0.8–v0.9** — MCP depth, webhooks, presence, share links: shipped.
- **v0.10** — Phase E: built-in agent runner, specialist board cards, @mention tasks, automations: shipped.
- **v0.11** — Phase F: local data sources and pull, copy/token push, SVG insert hooks: shipped.
- **v0.12** — Phase G: plugin execution, `omapaper://canvas/<id>` deep links, in-app update check: shipped.

Still open: website import, native menus and update button in the cockpit,
macOS/Windows packaging, task status history, scheduled email digests.

## 8. Explicitly out of scope

- Billing, subscriptions, seats, workspaces, roles
- Cloud accounts/login
- Hosted multi-tenant service
- Proprietary Figma/Canva/PSD cloud-only integrations unless open-source equivalents exist

## 9. Notes for implementation

- Keep the single SQLite database as source of truth.
- Prefer extending the existing frame HTML model instead of introducing a separate vector document format.
- Reuse MCP tools as the agent API; new UI capabilities should be exposed to agents too.
- Preserve Omarchy theming and local-first behavior.
- Maintain `cargo build/clippy/test` and `node --check canvas/app.js` on every change.
```