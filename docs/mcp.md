# OmaPaper MCP tools

OmaPaper exposes **46 tools** over MCP, both as a stdio server (`omapaper-mcp`) and over HTTP at `http://127.0.0.1:4310/mcp` (JSON-RPC). The daemon must be running.

Call `get_guide` once per session before any other tool. Topics: `omapaper-instructions` (default), `workflow`, `html`, `tokens`, `prototypes`, `comments`.

## Canvas

| Tool | Purpose |
|:-----|:--------|
| `get_profile` | Local identity and versions: no account, no cloud, no subscription. |
| `list_canvases` | Every canvas in the local database, most recently updated first. |
| `create_canvas` | Create a canvas with one page. Returns the canvas and its first page id. |
| `get_canvas` | Canvas map: pages, frames (id, name, size, approval), tokens, themes, guidelines. Call before designing. |
| `get_selection` | What the human currently has selected (page, frame, node). |
| `set_status` | One line shown next to your name on the canvas while you work. |
| `get_presence` | Who is on the canvas right now: humans, tabs and agents, with status lines. |

## Pages

| Tool | Purpose |
|:-----|:--------|
| `create_page` | Add a page to a canvas. |
| `set_canvas_pages` | Rename, reorder or remove pages in one call. Frames on a removed page move to the first page. |
| `set_page_background` | Colour behind a page's frames; omit or null for the theme paper. |

## Frames

| Tool | Purpose |
|:-----|:--------|
| `create_frame` | New HTML artboard on a page (phones 390×844, desktop 1440×900, cards 480×600). |
| `import_html` | Create a frame from a raw HTML document or a local HTML file (e.g. an export from Paper). |
| `get_frame` | One frame with its HTML, size, approval, element count and saved-version count. |
| `set_frame_html` | Replace a frame's whole document. Files the previous design as a version first. |
| `append_frame_html` | Stream a design: `start=true` begins a fresh document, each call appends a section, `done=true` finalises. |
| `edit_frame_html` | Exact find/replace inside a frame document. |
| `write_html` | Paper-style entry point: writes HTML into `frameId`, or creates a frame named `name` on `canvasId`. |
| `update_frame` | Rename, move, resize, retheme or set approval (`draft`, `needs_review`, `ready_for_dev`). |
| `delete_frame` | Delete a frame and its versions. |
| `duplicate_frame` | Copy a frame (HTML, size, background, theme) onto the same page, right after it. |
| `list_frame_versions` | Earlier states of a frame, newest first (last 30 kept). |
| `restore_frame_version` | Bring back a version's HTML and size; the current state is filed first. |
| `get_frame_screenshot` | Render the frame in headless Chromium and return a PNG. |
| `export_frame` | Write a frame to the export folder as `html`, `png` or `pdf`. Returns the absolute path. |

## Nodes

| Tool | Purpose |
|:-----|:--------|
| `get_tree_summary` | Element tree of a frame (body down) with ids, names, classes and text. |
| `find_nodes` | Elements matching a CSS selector (tag, `#id`, `.class`, `[attr]`, `[attr=value]`). |
| `get_node_info` | One element by `data-op-id` or selector: tag, attributes, inline styles, direct text, child count. |
| `get_children` | Direct children of an element (or of body). |
| `update_styles` | Merge inline styles into an element (camelCase or kebab-case keys; empty value removes). |
| `set_text_content` | Replace an element's text, keeping the element and its styles. |
| `rename_nodes` | Set a layer name (`data-name`) on an element. |
| `delete_nodes` | Delete matching elements. |
| `get_computed_styles` | Inline styles merged with matching `<style>` rules, as camelCase properties. |
| `get_jsx` | The frame body as JSX with `className`, `htmlFor` and style objects. |

## Tokens

| Tool | Purpose |
|:-----|:--------|
| `get_tokens` | Canvas design tokens, as `json`, `css` or `tailwind`. |
| `create_tokens` | Add or replace design tokens (typed CSS custom properties, one value per theme). |
| `set_tokens` | Change, rename or delete existing tokens. `replace` swaps the whole set. |
| `set_token_themes` | Declare the token themes in display order; the first is the default. |

## Comments

| Tool | Purpose |
|:-----|:--------|
| `get_comments` | Comment threads on a canvas, optionally for one frame. |
| `reply_to_comment` | Answer a comment thread as your agent name. |
| `resolve_comment` | Close or reopen a comment thread. |

## Guides & decisions

| Tool | Purpose |
|:-----|:--------|
| `get_guide` | The rules of designing on an OmaPaper canvas. |
| `get_guidelines` | Canvas style guides (brand rules, recipes). |
| `set_guidelines` | Create or replace a named style guide. |
| `save_decision` | Record a design decision humans made, so the canvas remembers their taste. |
| `finish_working` | Clear your cursor and status, and summarise the work in one line. |

## REST & WebSocket (daemon)

- `GET /` — canvas UI
- `GET /app.css`, `GET /app.js` — canvas assets
- `GET /health` — health and version
- `GET /api/info` — name, version, theme, themes, renderer, dataDir, protocol
- `GET /api/themes`, `GET /api/theme`, `GET /theme.css`
- `GET /api/canvases`, `GET /api/canvases/{id}`
- `GET/PUT /api/frames/{id}/html`, `GET /api/frames/{id}/versions`, `GET /api/frames/{id}/screenshot`, `GET /api/frames/{id}/export`
- `GET/POST /api/comments/{id}`
- `GET /mcp` — MCP over HTTP (JSON-RPC)
- `WS /ws` — presence, cursors, live edits
- `GET /api/presence` (pruned automatically)

## Notes

- Frames are whole HTML documents; keep `data-op-id` attributes intact so node tools and comments can address elements.
- Prototypes use `data-omapaper-on` and `data-omapaper-action` attributes.
- Screenshots/PDF need Chromium (`OMAPPAPER_CHROMIUM` to override the binary).
- Data lives under `~/.local/share/omapaper/` (override with `--data-dir` / `OMAPPAPER_DATA_DIR`).
