//! Every OmaPaper MCP tool.
//!
//! The list is the union of what Doop exposes (frames, pages, streaming,
//! versions, comments, presence) and what Paper exposes (a node tree, styles,
//! computed values, JSX, tokens, screenshots, export) — one canvas, one set of
//! names, no duplicated concepts.

use crate::render::base64;
use crate::Host;
use omapaper_core::html;
use omapaper_core::model::{
    color_for, new_id, now_ms, Approval, BoardCard, Canvas, CardRole, CardStatus, Comment, Frame,
    Guideline, Peer, PeerKind, Reply, Selection, Token, TokenKind,
};
use omapaper_core::{render, theme::Theme, tokens as core_tokens};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The MCP `tools/list` payload.
pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "get_guide",
            "Get guide",
            "The rules of designing on an OmaPaper canvas. Call once per session before any other tool; topics: omapaper-instructions (default), workflow, html, tokens, prototypes, comments.",
            json!({"topic": {"type": "string", "enum": omapaper_core::guide::TOPICS}}),
            &[],
        ),
        tool(
            "get_profile",
            "Get profile",
            "The local OmaPaper identity and versions: no account, no cloud, no subscription.",
            json!({}),
            &[],
        ),
        tool(
            "list_canvases",
            "List canvases",
            "Every canvas in the local database, most recently updated first.",
            json!({}),
            &[],
        ),
        tool(
            "create_canvas",
            "Create canvas",
            "Create a canvas with one page. Returns the canvas and its first page id.",
            json!({"name": {"type": "string", "description": "Canvas name, e.g. \"Travel app\"."}}),
            &["name"],
        ),
        tool(
            "get_canvas",
            "Get canvas",
            "The canvas map: pages, frames (id, name, size, approval), design tokens, themes and guidelines. Call this before designing. Omit canvasId when only one canvas exists.",
            json!({
                "canvasId": {"type": "string"},
                "approvalStatus": {"type": "string", "enum": ["draft", "needs_review", "ready_for_dev"]}
            }),
            &[],
        ),
        tool(
            "get_selection",
            "Get selection",
            "What the human currently has selected in the canvas tab (page, frame, node). Use it to design where they are looking.",
            json!({"canvasId": {"type": "string"}}),
            &[],
        ),
        tool(
            "set_status",
            "Set status",
            "One line shown next to your name on the canvas while you work. Reuse the same agentName on every call.",
            json!({
                "canvasId": {"type": "string"},
                "agentName": {"type": "string"},
                "status": {"type": "string"},
                "frameId": {"type": "string"}
            }),
            &["agentName", "status"],
        ),
        tool(
            "get_presence",
            "Get presence",
            "Who is on the canvas right now: humans, their tabs and agents, with status lines.",
            json!({"canvasId": {"type": "string"}}),
            &[],
        ),
        tool(
            "create_page",
            "Create page",
            "Add a page to a canvas (frames live on pages).",
            json!({"canvasId": {"type": "string"}, "name": {"type": "string"}}),
            &["name"],
        ),
        tool(
            "set_canvas_pages",
            "Set canvas pages",
            "Rename, reorder or remove pages in one call. Pass the pages you want, in order; unknown ids create new pages. Frames on a removed page move to the first page.",
            json!({
                "canvasId": {"type": "string"},
                "pages": {"type": "array", "items": {"type": "object", "properties": {
                    "id": {"type": "string"}, "name": {"type": "string"}
                }, "required": ["name"]}}
            }),
            &["pages"],
        ),
        tool(
            "set_page_background",
            "Set page background",
            "The colour behind a page's frames; omit or pass null for the theme paper.",
            json!({
                "canvasId": {"type": "string"},
                "pageId": {"type": "string"},
                "background": {"type": ["string", "null"]}
            }),
            &["pageId"],
        ),
        tool(
            "create_frame",
            "Create frame",
            "A new HTML artboard on a page. Phones are 390×844, desktop 1440×900, cards 480×600.",
            json!({
                "canvasId": {"type": "string"},
                "pageId": {"type": "string"},
                "name": {"type": "string"},
                "width": {"type": "number"},
                "height": {"type": "number"},
                "theme": {"type": "string", "description": "light or dark, applied as data-omapaper-theme."}
            }),
            &[],
        ),
        tool(
            "import_html",
            "Import HTML",
            "Create a frame from a raw HTML document or a local HTML file (e.g. an export from Paper).",
            json!({
                "canvasId": {"type": "string"},
                "pageId": {"type": "string"},
                "name": {"type": "string"},
                "width": {"type": "number"},
                "height": {"type": "number"},
                "html": {"type": "string", "description": "Raw HTML document."},
                "path": {"type": "string", "description": "Path to a local .html file."}
            }),
            &[],
        ),
        tool(
            "get_frame",
            "Get frame",
            "One frame with its HTML, size, approval, element count and the number of saved versions.",
            json!({"frameId": {"type": "string"}}),
            &["frameId"],
        ),
        tool(
            "set_frame_html",
            "Set frame HTML",
            "Replace a frame's whole document. Files the previous design as a version first.",
            json!({
                "frameId": {"type": "string"},
                "html": {"type": "string"},
                "label": {"type": "string", "description": "Version label, e.g. \"before pricing rework\"."}
            }),
            &["frameId", "html"],
        ),
        tool(
            "append_frame_html",
            "Append frame HTML (stream)",
            "Stream a design: start=true begins a fresh document (previous one is versioned), each call appends a section, done=true finalises. Viewers watch every chunk appear.",
            json!({
                "frameId": {"type": "string"},
                "html": {"type": "string"},
                "start": {"type": "boolean"},
                "done": {"type": "boolean"}
            }),
            &["frameId", "html"],
        ),
        tool(
            "edit_frame_html",
            "Edit frame HTML",
            "Exact find/replace inside a frame document, for small changes that should not disturb the rest.",
            json!({
                "frameId": {"type": "string"},
                "find": {"type": "string"},
                "replace": {"type": "string"},
                "all": {"type": "boolean", "description": "Replace every occurrence (default: the first)."}
            }),
            &["frameId", "find", "replace"],
        ),
        tool(
            "write_html",
            "Write HTML",
            "Paper-style entry point: writes HTML into frameId, or creates a frame named `name` on canvasId and writes there. Replaces the document.",
            json!({
                "frameId": {"type": "string"},
                "canvasId": {"type": "string"},
                "name": {"type": "string"},
                "html": {"type": "string"}
            }),
            &["html"],
        ),
        tool(
            "update_frame",
            "Update frame",
            "Rename, move, resize, retheme or set approval (draft, needs_review, ready_for_dev). Size or HTML changes drop approval back to draft.",
            json!({
                "frameId": {"type": "string"},
                "name": {"type": "string"},
                "x": {"type": "number"},
                "y": {"type": "number"},
                "width": {"type": "number"},
                "height": {"type": "number"},
                "theme": {"type": ["string", "null"]},
                "background": {"type": ["string", "null"]},
                "approval": {"type": "string", "enum": ["draft", "needs_review", "ready_for_dev"]},
                "approvalBy": {"type": "string"},
                "locked": {"type": "boolean"}
            }),
            &["frameId"],
        ),
        tool(
            "delete_frame",
            "Delete frame",
            "Delete a frame and its versions.",
            json!({"frameId": {"type": "string"}}),
            &["frameId"],
        ),
        tool(
            "duplicate_frame",
            "Duplicate frame",
            "Copy a frame (HTML, size, background, theme) onto the same page, right after it.",
            json!({"frameId": {"type": "string"}, "name": {"type": "string"}}),
            &["frameId"],
        ),
        tool(
            "list_frame_versions",
            "List frame versions",
            "Earlier states of a frame, newest first. The daemon keeps the last 30.",
            json!({"frameId": {"type": "string"}}),
            &["frameId"],
        ),
        tool(
            "restore_frame_version",
            "Restore frame version",
            "Bring back a version's HTML and size; the current state is filed first.",
            json!({"frameId": {"type": "string"}, "versionId": {"type": "string"}}),
            &["frameId", "versionId"],
        ),
        tool(
            "get_frame_screenshot",
            "Get frame screenshot",
            "Render the frame in headless Chromium and return a PNG to look at. Always review a design this way before calling it done.",
            json!({
                "frameId": {"type": "string"},
                "scale": {"type": "number", "description": "Device scale factor, 0.25–3 (default 1)."}
            }),
            &["frameId"],
        ),
        tool(
            "export_frame",
            "Export frame",
            "Write a frame to the export folder as html, png or pdf. Returns the absolute path.",
            json!({
                "frameId": {"type": "string"},
                "format": {"type": "string", "enum": ["html", "png", "pdf"]}
            }),
            &["frameId"],
        ),
        tool(
            "get_tree_summary",
            "Get tree summary",
            "The element tree of a frame (body down), with ids, names, classes and text — the map for node tools.",
            json!({"frameId": {"type": "string"}, "depth": {"type": "integer", "minimum": 0}}),
            &["frameId"],
        ),
        tool(
            "find_nodes",
            "Find nodes",
            "Elements matching a CSS selector. Supported: tag, #id, .class, [attr], [attr=\"value\"] and descendant spaces; no pseudo-classes.",
            json!({"frameId": {"type": "string"}, "selector": {"type": "string"}}),
            &["frameId", "selector"],
        ),
        tool(
            "get_node_info",
            "Get node info",
            "One element by data-op-id or selector: tag, attributes, inline styles, direct text and child count.",
            json!({
                "frameId": {"type": "string"},
                "nodeId": {"type": "string"},
                "selector": {"type": "string"}
            }),
            &["frameId"],
        ),
        tool(
            "get_children",
            "Get children",
            "The direct children of an element (or of body), each with tag, id, name and text.",
            json!({
                "frameId": {"type": "string"},
                "nodeId": {"type": "string"},
                "selector": {"type": "string"}
            }),
            &["frameId"],
        ),
        tool(
            "update_styles",
            "Update styles",
            "Merge inline styles into an element (camelCase or kebab-case keys). An empty value removes the property.",
            json!({
                "frameId": {"type": "string"},
                "nodeId": {"type": "string"},
                "nodeIds": {"type": "array", "items": {"type": "string"}, "description": "Apply to these data-op-id values."},
                "selector": {"type": "string"},
                "styles": {"type": "object", "additionalProperties": {"type": ["string", "number"]}},
                "index": {"type": "integer"},
                "all": {"type": "boolean", "description": "Apply to every match instead of one."}
            }),
            &["frameId", "styles"],
        ),
        tool(
            "set_text_content",
            "Set text content",
            "Replace an element's text, keeping the element and its styles.",
            json!({
                "frameId": {"type": "string"},
                "nodeId": {"type": "string"},
                "nodeIds": {"type": "array", "items": {"type": "string"}, "description": "Apply to these data-op-id values."},
                "selector": {"type": "string"},
                "text": {"type": "string"},
                "index": {"type": "integer"},
                "all": {"type": "boolean", "description": "Apply to every match instead of one."}
            }),
            &["frameId", "text"],
        ),
        tool(
            "rename_nodes",
            "Rename nodes",
            "Set a layer name (data-name) on an element; the HTML structure is untouched.",
            json!({
                "frameId": {"type": "string"},
                "nodeId": {"type": "string"},
                "nodeIds": {"type": "array", "items": {"type": "string"}, "description": "Apply to these data-op-id values."},
                "selector": {"type": "string"},
                "name": {"type": "string"},
                "index": {"type": "integer"},
                "all": {"type": "boolean", "description": "Apply to every match instead of one."}
            }),
            &["frameId", "name"],
        ),
        tool(
            "delete_nodes",
            "Delete nodes",
            "Delete matching elements from the document.",
            json!({
                "frameId": {"type": "string"},
                "nodeId": {"type": "string"},
                "nodeIds": {"type": "array", "items": {"type": "string"}, "description": "Apply to these data-op-id values."},
                "selector": {"type": "string"},
                "index": {"type": "integer"},
                "all": {"type": "boolean", "description": "Apply to every match instead of one."}
            }),
            &["frameId"],
        ),
        tool(
            "get_computed_styles",
            "Get computed styles",
            "Inline styles merged with matching <style> rules, as camelCase properties. Inheritance is not resolved.",
            json!({
                "frameId": {"type": "string"},
                "nodeId": {"type": "string"},
                "selector": {"type": "string"}
            }),
            &["frameId"],
        ),
        tool(
            "get_jsx",
            "Get JSX",
            "The frame body as JSX with className, htmlFor and style objects — for handing a design to a codebase.",
            json!({"frameId": {"type": "string"}}),
            &["frameId"],
        ),
        tool(
            "get_tokens",
            "Get tokens",
            "The canvas design tokens, as json, css or tailwind.",
            json!({
                "canvasId": {"type": "string"},
                "format": {"type": "string", "enum": ["json", "css", "tailwind", "markdown"]}
            }),
            &[],
        ),
        tool(
            "create_tokens",
            "Create tokens",
            "Add or replace design tokens (typed CSS custom properties with one value per theme).",
            json!({
                "canvasId": {"type": "string"},
                "tokens": {"type": "array", "items": {"type": "object", "properties": {
                    "name": {"type": "string"},
                    "kind": {"type": "string", "enum": ["color", "spacing", "radius", "typography", "shadow", "other"]},
                    "description": {"type": "string"},
                    "values": {"type": "object", "additionalProperties": {"type": "string"}}
                }, "required": ["name", "values"]}}
            }),
            &["tokens"],
        ),
        tool(
            "set_tokens",
            "Set tokens",
            "Change, rename or delete existing tokens. `replace` swaps the whole set.",
            json!({
                "canvasId": {"type": "string"},
                "tokens": {"type": "array", "items": {"type": "object"}},
                "delete": {"type": "array", "items": {"type": "string"}},
                "replace": {"type": "boolean"}
            }),
            &[],
        ),
        tool(
            "set_token_themes",
            "Set token themes",
            "Declare the token themes in display order; the first is the default.",
            json!({
                "canvasId": {"type": "string"},
                "themes": {"type": "array", "items": {"type": "string"}}
            }),
            &["themes"],
        ),
        tool(
            "get_comments",
            "Get comments",
            "Comment threads on a canvas, optionally for one frame.",
            json!({"canvasId": {"type": "string"}, "frameId": {"type": "string"}}),
            &[],
        ),
        tool(
            "reply_to_comment",
            "Reply to comment",
            "Answer a comment thread as your agent name.",
            json!({
                "commentId": {"type": "string"},
                "body": {"type": "string"},
                "author": {"type": "string"}
            }),
            &["commentId", "body"],
        ),
        tool(
            "resolve_comment",
            "Resolve comment",
            "Close (or reopen) a comment thread.",
            json!({"commentId": {"type": "string"}, "resolved": {"type": "boolean"}}),
            &["commentId"],
        ),
        tool(
            "get_guidelines",
            "Get guidelines",
            "The canvas style guides (brand rules, recipes). Read the relevant one before designing.",
            json!({"canvasId": {"type": "string"}}),
            &[],
        ),
        tool(
            "set_guidelines",
            "Set guidelines",
            "Create or replace a named style guide on the canvas.",
            json!({
                "canvasId": {"type": "string"},
                "name": {"type": "string"},
                "body": {"type": "string"}
            }),
            &["name", "body"],
        ),
        tool(
            "save_decision",
            "Save decision",
            "Record a design decision the humans made, so the canvas remembers their taste.",
            json!({"canvasId": {"type": "string"}, "body": {"type": "string"}}),
            &["body"],
        ),
        tool(
            "finish_working",
            "Finish working",
            "Clear your cursor and status, and summarise the work in one line.",
            json!({
                "canvasId": {"type": "string"},
                "agentName": {"type": "string"},
                "summary": {"type": "string"}
            }),
            &["agentName"],
        ),
        tool(
            "list_data_sources",
            "List data sources",
            "The local JSON, CSV or .env files a canvas can pull real content from.",
            json!({"canvasId": {"type": "string"}}),
            &[],
        ),
        tool(
            "pull_data",
            "Pull data",
            "Replace every {{dotted.key}} in the canvas frames with values from its data sources. Returns how many frames and placeholders changed.",
            json!({"canvasId": {"type": "string"}, "sourceId": {"type": "string"}}),
            &[],
        ),
        tool(
            "push_sync",
            "Push sync",
            "Write a frame element's text (or the canvas tokens as CSS) to a sync target file.",
            json!({"canvasId": {"type": "string"}, "syncId": {"type": "string"}}),
            &["syncId"],
        ),
        tool(
            "list_board_cards",
            "List board cards",
            "Tasks on the canvas board: brief, specialist role, status and what came back.",
            json!({"canvasId": {"type": "string"}}),
            &[],
        ),
        tool(
            "create_board_card",
            "Create board card",
            "Queue a task for a specialist (design, copy, code, review, research). Humans can run it from the Agents tab or @mention a role in a comment.",
            json!({
                "canvasId": {"type": "string"},
                "title": {"type": "string"},
                "brief": {"type": "string"},
                "role": {"type": "string", "enum": ["design", "copy", "code", "review", "research"]},
                "frameId": {"type": "string"},
                "author": {"type": "string"}
            }),
            &["brief"],
        ),
        tool(
            "update_board_card",
            "Update board card",
            "Claim a card (status working), finish it (done, with a result) or report a failure.",
            json!({
                "canvasId": {"type": "string"},
                "cardId": {"type": "string"},
                "status": {"type": "string", "enum": ["queued", "working", "done", "failed"]},
                "result": {"type": "string"},
                "title": {"type": "string"},
                "brief": {"type": "string"}
            }),
            &["cardId"],
        ),
    ]
}

fn tool(name: &str, title: &str, description: &str, props: Value, required: &[&str]) -> Value {
    let mut schema = serde_json::Map::new();
    schema.insert("type".into(), "object".into());
    schema.insert("properties".into(), props);
    schema.insert("additionalProperties".into(), false.into());
    if !required.is_empty() {
        schema.insert("required".into(), json!(required));
    }
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": Value::Object(schema)
    })
}

// ---- dispatch -------------------------------------------------------------

/// Runs a tool, returning MCP content items.
pub fn call(host: &Host, name: &str, args: &Value) -> Result<Vec<Value>, String> {
    let ctx = Ctx { host, args };
    match name {
        "get_guide" => {
            let topic = ctx.opt_str("topic").unwrap_or_default();
            let chosen = if topic.is_empty() {
                "omapaper-instructions"
            } else {
                topic.as_str()
            };
            Ok(text(json!({
                "topic": chosen,
                "guide": omapaper_core::guide::topic(chosen)
            })))
        }
        "get_profile" => Ok(text(json!({
            "name": "OmaPaper",
            "version": omapaper_core::VERSION,
            "server": "omapaperd",
            "local": true,
            "free": true,
            "renderer": host.renderer.name(),
        }))),
        "list_canvases" => Ok(text(json!(host.store.list_canvases().map_err(err)?))),
        "create_canvas" => {
            let name = ctx.req_str("name")?;
            let canvas = host.store.create_canvas(&name).map_err(err)?;
            host.emit(
                &canvas.id,
                json!({"type": "canvas.created", "canvas": canvas}),
            );
            Ok(text(json!(canvas)))
        }
        "get_canvas" => {
            let canvas = ctx.canvas()?;
            let mut frames = host.store.frames(&canvas.id).map_err(err)?;
            if let Some(status) = ctx.opt_str("approvalStatus") {
                frames.retain(|f| approval_key(f.approval) == status);
            }
            let frames: Vec<Value> = frames
                .iter()
                .map(|frame| {
                    json!({
                        "id": frame.id,
                        "pageId": frame.page_id,
                        "name": frame.name,
                        "width": frame.width,
                        "height": frame.height,
                        "approval": approval_key(frame.approval),
                        "approvalBy": frame.approval_by,
                        "updatedAt": frame.updated_at,
                        "elements": html::element_count(&frame.html),
                    })
                })
                .collect();
            Ok(text(json!({
                "canvas": {
                    "id": canvas.id,
                    "name": canvas.name,
                    "pages": canvas.pages,
                    "tokenThemes": canvas.theme_names(),
                },
                "frames": frames,
                "tokens": canvas.tokens,
                "guidelines": canvas.guidelines.iter().map(|g| json!({
                    "name": g.name, "summary": first_line(&g.body)
                })).collect::<Vec<_>>(),
                "decisions": canvas.decisions,
                "renderer": host.renderer.name(),
            })))
        }
        "get_selection" => {
            let canvas = ctx.canvas()?;
            let selection = host
                .presence
                .selection(&canvas.id)
                .unwrap_or_else(|| Selection {
                    canvas_id: canvas.id.clone(),
                    ..Selection::default()
                });
            Ok(text(json!(selection)))
        }
        "set_status" => {
            let canvas = ctx.canvas()?;
            let name = ctx.req_str("agentName")?;
            let status = ctx.req_str("status")?;
            host.presence.set(
                &canvas.id,
                Peer {
                    id: format!("agent-{name}"),
                    name: name.clone(),
                    kind: PeerKind::Agent,
                    color: color_for(&name),
                    frame_id: ctx.opt_str("frameId"),
                    x: 0.0,
                    y: 0.0,
                    status: Some(status.clone()),
                    selection: Vec::new(),
                    updated_at: now_ms(),
                },
            );
            Ok(text(json!({"ok": true, "agent": name, "status": status})))
        }
        "get_presence" => {
            let canvas = ctx.canvas()?;
            Ok(text(json!(host.presence.list(&canvas.id))))
        }
        "create_page" => {
            let mut canvas = ctx.canvas()?;
            let name = ctx.req_str("name")?;
            let page = omapaper_core::Page::new(name);
            canvas.pages.push(page.clone());
            host.store.save_canvas(&canvas).map_err(err)?;
            host.emit(
                &canvas.id,
                json!({"type": "canvas.updated", "canvas": canvas}),
            );
            Ok(text(json!(page)))
        }
        "set_canvas_pages" => {
            let mut canvas = ctx.canvas()?;
            let pages = ctx
                .args
                .get("pages")
                .and_then(Value::as_array)
                .ok_or("`pages` must be an array")?;
            let mut next = Vec::new();
            for page in pages {
                let name = page
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("every page needs a `name`")?;
                let existing = page
                    .get("id")
                    .and_then(Value::as_str)
                    .and_then(|id| canvas.pages.iter().find(|p| p.id == id));
                match existing {
                    Some(page) => next.push(omapaper_core::Page {
                        name: name.to_string(),
                        ..page.clone()
                    }),
                    None => next.push(omapaper_core::Page::new(name)),
                }
            }
            if next.is_empty() {
                return Err("a canvas needs at least one page".into());
            }
            let keep: Vec<String> = next.iter().map(|p| p.id.clone()).collect();
            canvas.pages = next;
            host.store.save_canvas(&canvas).map_err(err)?;
            // Move orphaned frames to the first page.
            let first_page = canvas.pages[0].id.clone();
            for mut frame in host.store.frames(&canvas.id).map_err(err)? {
                if !keep.contains(&frame.page_id) {
                    frame.page_id = first_page.clone();
                    host.store.save_frame(&frame).map_err(err)?;
                }
            }
            host.emit(
                &canvas.id,
                json!({"type": "canvas.updated", "canvas": canvas}),
            );
            Ok(text(json!(canvas)))
        }
        "set_page_background" => {
            let mut canvas = ctx.canvas()?;
            let page_id = ctx.req_str("pageId")?;
            let background = ctx.opt_str("background");
            let Some(page) = canvas.pages.iter_mut().find(|p| p.id == page_id) else {
                return Err(format!("no page `{page_id}`"));
            };
            page.background = background;
            host.store.save_canvas(&canvas).map_err(err)?;
            host.emit(
                &canvas.id,
                json!({"type": "canvas.updated", "canvas": canvas}),
            );
            Ok(text(json!(canvas.pages)))
        }
        "create_frame" => {
            let canvas = ctx.canvas()?;
            let page_id = match ctx.opt_str("pageId") {
                Some(page_id) => {
                    if canvas.page(&page_id).is_none() {
                        return Err(format!("no page `{page_id}`"));
                    }
                    page_id
                }
                None => canvas.pages[0].id.clone(),
            };
            let count = host.store.frames(&canvas.id).map_err(err)?.len() + 1;
            let name = ctx
                .opt_str("name")
                .unwrap_or_else(|| format!("Frame {count}"));
            let mut frame = Frame::new(&canvas.id, &page_id, name);
            if let Some(width) = ctx.opt_f64("width") {
                frame.width = width;
            }
            if let Some(height) = ctx.opt_f64("height") {
                frame.height = height;
            }
            frame.theme = ctx.opt_str("theme");
            host.store.insert_frame(&frame).map_err(err)?;
            host.emit(&canvas.id, json!({"type": "frame.created", "frame": frame}));
            Ok(text(json!({"frame": frame})))
        }
        "import_html" => {
            let canvas = ctx.canvas()?;
            let page_id = match ctx.opt_str("pageId") {
                Some(page_id) => {
                    if canvas.page(&page_id).is_none() {
                        return Err(format!("no page `{page_id}`"));
                    }
                    page_id
                }
                None => canvas.pages[0].id.clone(),
            };
            let html = if let Some(path) = ctx.opt_str("path") {
                std::fs::read_to_string(&path).map_err(|e| format!("cannot read `{path}`: {e}"))?
            } else {
                ctx.req_str("html")?
            };
            let count = host.store.frames(&canvas.id).map_err(err)?.len() + 1;
            let name = ctx
                .opt_str("name")
                .unwrap_or_else(|| format!("Imported Frame {count}"));
            let mut frame = Frame::new(&canvas.id, &page_id, name);
            if let Some(width) = ctx.opt_f64("width") {
                frame.width = width;
            }
            if let Some(height) = ctx.opt_f64("height") {
                frame.height = height;
            }
            frame.html = html;
            frame.revision = 1;
            host.store.insert_frame(&frame).map_err(err)?;
            host.emit(&canvas.id, json!({"type": "frame.created", "frame": frame}));
            Ok(text(json!({"frame": frame, "elements": html::element_count(&frame.html)})))
        }
        "get_frame" => {
            let frame = ctx.frame()?;
            let versions = host.store.versions(&frame.id).map_err(err)?.len();
            Ok(text(json!({
                "frame": frame,
                "elements": html::element_count(&frame.html),
                "versions": versions
            })))
        }
        "set_frame_html" => {
            let mut frame = ctx.frame()?;
            ensure_unlocked(&frame)?;
            let new_html = ctx.req_str("html")?;
            let label = ctx.opt_str("label");
            host.store
                .push_version(
                    &frame,
                    label.as_deref().or(Some("before set_frame_html")),
                    30,
                )
                .map_err(err)?;
            frame.html = new_html;
            frame.revision += 1;
            frame.approval = Approval::Draft;
            frame.approval_by = None;
            host.save_frame(&frame)?;
            Ok(text(
                json!({"frame": frame, "elements": html::element_count(&frame.html)}),
            ))
        }
        "append_frame_html" => {
            let mut frame = ctx.frame()?;
            ensure_unlocked(&frame)?;
            let chunk = ctx.req_str("html")?;
            let start = ctx.opt_bool("start").unwrap_or(false);
            let done = ctx.opt_bool("done").unwrap_or(false);
            if start {
                host.store
                    .push_version(&frame, Some("before stream"), 30)
                    .map_err(err)?;
                frame.html.clear();
            }
            frame.html.push_str(&chunk);
            frame.revision += 1;
            frame.approval = Approval::Draft;
            frame.updated_at = now_ms();
            host.save_frame(&frame)?;
            let mut payload = json!({
                "frameId": frame.id,
                "revision": frame.revision,
                "bytes": frame.html.len(),
                "done": done
            });
            if done {
                payload["elements"] = html::element_count(&frame.html).into();
            }
            Ok(text(payload))
        }
        "edit_frame_html" => {
            let mut frame = ctx.frame()?;
            ensure_unlocked(&frame)?;
            let find = ctx.req_str("find")?;
            let replace = ctx.req_str("replace")?;
            let all = ctx.opt_bool("all").unwrap_or(false);
            if !frame.html.contains(&find) {
                return Err(format!(
                    "`find` does not occur in frame {} — read get_frame first",
                    frame.id
                ));
            }
            host.store
                .push_version(&frame, Some("before edit_frame_html"), 30)
                .map_err(err)?;
            let replaced = if all {
                let count = frame.html.matches(&find).count();
                frame.html = frame.html.replace(&find, &replace);
                count
            } else {
                frame.html = frame.html.replacen(&find, &replace, 1);
                1
            };
            frame.revision += 1;
            frame.approval = Approval::Draft;
            host.save_frame(&frame)?;
            Ok(text(
                json!({"frameId": frame.id, "replaced": replaced, "revision": frame.revision}),
            ))
        }
        "write_html" => {
            let html = ctx.req_str("html")?;
            let (mut frame, canvas_id) = match ctx.opt_str("frameId") {
                Some(id) => {
                    let frame = host
                        .store
                        .get_frame(&id)
                        .map_err(err)?
                        .ok_or_else(|| format!("no frame `{id}`"))?;
                    let canvas_id = frame.canvas_id.clone();
                    (frame, canvas_id)
                }
                None => {
                    let canvas = ctx.canvas()?;
                    let count = host.store.frames(&canvas.id).map_err(err)?.len() + 1;
                    let name = ctx
                        .opt_str("name")
                        .unwrap_or_else(|| format!("Frame {count}"));
                    let frame = Frame::new(&canvas.id, &canvas.pages[0].id, name);
                    host.store.insert_frame(&frame).map_err(err)?;
                    (frame, canvas.id)
                }
            };
            ensure_unlocked(&frame)?;
            host.store
                .push_version(&frame, Some("before write_html"), 30)
                .map_err(err)?;
            frame.html = html;
            frame.revision += 1;
            frame.approval = Approval::Draft;
            host.save_frame(&frame)?;
            host.touch_canvas(&canvas_id);
            Ok(text(
                json!({"frameId": frame.id, "canvasId": canvas_id, "elements": html::element_count(&frame.html)}),
            ))
        }
        "update_frame" => {
            let mut frame = ctx.frame()?;
            let mut resized = false;
            if let Some(name) = ctx.opt_str("name") {
                frame.name = name;
            }
            if let Some(x) = ctx.opt_f64("x") {
                frame.x = x;
            }
            if let Some(y) = ctx.opt_f64("y") {
                frame.y = y;
            }
            if let Some(width) = ctx.opt_f64("width") {
                resized |= (frame.width - width).abs() > f64::EPSILON;
                frame.width = width;
            }
            if let Some(height) = ctx.opt_f64("height") {
                resized |= (frame.height - height).abs() > f64::EPSILON;
                frame.height = height;
            }
            if ctx.args.get("theme").is_some() {
                frame.theme = ctx.opt_str("theme");
            }
            if ctx.args.get("background").is_some() {
                frame.background = ctx.opt_str("background");
            }
            if let Some(locked) = ctx.opt_bool("locked") {
                frame.locked = locked;
            }
            if let Some(approval) = ctx.opt_str("approval") {
                frame.approval = parse_approval(&approval)?;
                frame.approval_by = ctx.opt_str("approvalBy");
            }
            if resized {
                frame.approval = Approval::Draft;
            }
            host.save_frame(&frame)?;
            Ok(text(json!({"frame": frame})))
        }
        "delete_frame" => {
            let frame = ctx.frame()?;
            host.store.delete_frame(&frame.id).map_err(err)?;
            host.emit(
                &frame.canvas_id,
                json!({"type": "frame.deleted", "frameId": frame.id}),
            );
            Ok(text(json!({"ok": true, "frameId": frame.id})))
        }
        "duplicate_frame" => {
            let source = ctx.frame()?;
            let mut frame = Frame::new(
                &source.canvas_id,
                &source.page_id,
                ctx.opt_str("name")
                    .unwrap_or_else(|| format!("{} copy", source.name)),
            );
            frame.html = source.html;
            frame.width = source.width;
            frame.height = source.height;
            frame.background = source.background;
            frame.theme = source.theme;
            frame.x = source.x + 24.0;
            frame.y = source.y + 24.0;
            host.store.insert_frame(&frame).map_err(err)?;
            host.emit(
                &source.canvas_id,
                json!({"type": "frame.created", "frame": frame}),
            );
            Ok(text(json!(frame)))
        }
        "list_frame_versions" => {
            let frame = ctx.frame()?;
            let versions: Vec<Value> = host
                .store
                .versions(&frame.id)
                .map_err(err)?
                .into_iter()
                .map(|v| {
                    json!({
                        "id": v.id,
                        "label": v.label,
                        "width": v.width,
                        "height": v.height,
                        "bytes": v.html.len(),
                        "createdAt": v.created_at,
                    })
                })
                .collect();
            Ok(text(json!(versions)))
        }
        "restore_frame_version" => {
            let mut frame = ctx.frame()?;
            ensure_unlocked(&frame)?;
            let version_id = ctx.req_str("versionId")?;
            let version = host
                .store
                .version(&version_id)
                .map_err(err)?
                .filter(|v| v.frame_id == frame.id)
                .ok_or_else(|| format!("no version `{version_id}` on frame {}", frame.id))?;
            host.store
                .push_version(&frame, Some("before restore"), 30)
                .map_err(err)?;
            frame.html = version.html;
            frame.width = version.width;
            frame.height = version.height;
            frame.revision += 1;
            frame.approval = Approval::Draft;
            host.save_frame(&frame)?;
            Ok(text(json!({"frame": frame})))
        }
        "get_frame_screenshot" => {
            let frame = ctx.frame()?;
            let scale = ctx.opt_f64("scale").unwrap_or(1.0);
            if !host.renderer.available() {
                return Err("screenshots need Chromium on this machine (install chromium or set OMAPPAPER_CHROMIUM)".into());
            }
            let theme = Theme::live_omarchy().unwrap_or_default();
            let canvas = host.store.get_canvas(&frame.canvas_id).map_err(err)?;
            let tokens_css = canvas
                .map(|c| core_tokens::tokens_css_all(&c.tokens, &c.token_themes))
                .unwrap_or_default();
            let doc = render::frame_document(&frame, &theme, &tokens_css);
            let png = host
                .renderer
                .screenshot(&doc, frame.width, frame.height, scale)
                .map_err(|e| e.to_string())?;
            Ok(vec![
                json!({"type": "image", "data": base64(&png), "mimeType": "image/png"}),
                json!({"type": "text", "text": format!(
                    "Frame \"{}\" rendered at {}×{} ({} bytes PNG).",
                    frame.name, frame.width, frame.height, png.len()
                )}),
            ])
        }
        "export_frame" => {
            let frame = ctx.frame()?;
            let format = ctx.opt_str("format").unwrap_or_else(|| "html".into());
            let theme = Theme::live_omarchy().unwrap_or_default();
            let canvas = host.store.get_canvas(&frame.canvas_id).map_err(err)?;
            let tokens_css = canvas
                .map(|c| core_tokens::tokens_css_all(&c.tokens, &c.token_themes))
                .unwrap_or_default();
            let doc = render::frame_document(&frame, &theme, &tokens_css);
            let exports = host.data_dir.join("exports");
            let stem = format!("{}-{}", slug(&frame.name), frame.id);
            let path = match format.as_str() {
                "html" => host
                    .renderer
                    .write_export(&exports, &format!("{stem}.html"), doc.as_bytes())
                    .map_err(|e| e.to_string())?,
                "png" => {
                    let png = host
                        .renderer
                        .screenshot(&doc, frame.width, frame.height, 1.0)
                        .map_err(|e| e.to_string())?;
                    host.renderer
                        .write_export(&exports, &format!("{stem}.png"), &png)
                        .map_err(|e| e.to_string())?
                }
                "pdf" => {
                    let pdf = host
                        .renderer
                        .pdf(&doc, frame.width, frame.height)
                        .map_err(|e| e.to_string())?;
                    host.renderer
                        .write_export(&exports, &format!("{stem}.pdf"), &pdf)
                        .map_err(|e| e.to_string())?
                }
                other => return Err(format!("unsupported export format `{other}`")),
            };
            Ok(text(json!({"path": path, "format": format})))
        }
        "get_tree_summary" => {
            let frame = ctx.frame()?;
            let depth = ctx.opt_u64("depth").unwrap_or(4) as usize;
            Ok(text(html::tree_json(&frame.html, depth.min(20))))
        }
        "find_nodes" => {
            let frame = ctx.frame()?;
            let selector = ctx.req_str("selector")?;
            let nodes = html::find_json(&frame.html, &selector).map_err(|e| e.to_string())?;
            Ok(text(json!({"count": nodes.len(), "nodes": nodes})))
        }
        "get_node_info" => {
            let frame = ctx.frame()?;
            let selector = ctx.selector()?;
            let value = html::node_json(&frame.html, &selector).map_err(|e| e.to_string())?;
            Ok(text(value))
        }
        "get_children" => {
            let frame = ctx.frame()?;
            let selector = match ctx.opt_str("nodeId") {
                Some(id) => html::selector_for_id(&id),
                None => ctx.opt_str("selector").unwrap_or_else(|| "body".into()),
            };
            let children =
                html::children_json(&frame.html, &selector).map_err(|e| e.to_string())?;
            Ok(text(json!({"count": children.len(), "children": children})))
        }
        "update_styles" => {
            let mut frame = ctx.frame()?;
            ensure_unlocked(&frame)?;
            let styles = ctx.style_map("styles")?;
            let index = ctx.opt_u64("index").map(|i| i as usize);
            let all = ctx.opt_bool("all").unwrap_or(false);
            let (updated, count) = if let Some(ids) = ctx.opt_str_vec("nodeIds") {
                if ids.is_empty() {
                    return Err("`nodeIds` must not be empty".into());
                }
                let mut html = frame.html.clone();
                let mut count = 0usize;
                for id in ids {
                    let selector = html::selector_for_id(&id);
                    let (u, c) = html::update_styles(&html, &selector, &styles, None, false)
                        .map_err(|e| e.to_string())?;
                    html = u;
                    count += c;
                }
                (html, count)
            } else {
                let selector = ctx.selector()?;
                html::update_styles(&frame.html, &selector, &styles, index, all)
                    .map_err(|e| e.to_string())?
            };
            host.store
                .push_version(&frame, Some("before update_styles"), 30)
                .map_err(err)?;
            frame.html = updated;
            frame.revision += 1;
            frame.approval = Approval::Draft;
            host.save_frame(&frame)?;
            Ok(text(json!({"frameId": frame.id, "updated": count})))
        }
        "set_text_content" => {
            let mut frame = ctx.frame()?;
            ensure_unlocked(&frame)?;
            let body = ctx.req_str("text")?;
            let index = ctx.opt_u64("index").map(|i| i as usize);
            let all = ctx.opt_bool("all").unwrap_or(false);
            let updated = if let Some(ids) = ctx.opt_str_vec("nodeIds") {
                if ids.is_empty() {
                    return Err("`nodeIds` must not be empty".into());
                }
                let mut html = frame.html.clone();
                for id in ids {
                    let selector = html::selector_for_id(&id);
                    html = html::set_text(&html, &selector, &body, None, false)
                        .map_err(|e| e.to_string())?;
                }
                html
            } else {
                let selector = ctx.selector()?;
                html::set_text(&frame.html, &selector, &body, index, all).map_err(|e| e.to_string())?
            };
            host.store
                .push_version(&frame, Some("before set_text_content"), 30)
                .map_err(err)?;
            frame.html = updated;
            frame.revision += 1;
            frame.approval = Approval::Draft;
            host.save_frame(&frame)?;
            Ok(text(json!({"frameId": frame.id, "text": body})))
        }
        "rename_nodes" => {
            let mut frame = ctx.frame()?;
            ensure_unlocked(&frame)?;
            let name = ctx.req_str("name")?;
            let index = ctx.opt_u64("index").map(|i| i as usize);
            let all = ctx.opt_bool("all").unwrap_or(false);
            let updated = if let Some(ids) = ctx.opt_str_vec("nodeIds") {
                if ids.is_empty() {
                    return Err("`nodeIds` must not be empty".into());
                }
                let mut html = frame.html.clone();
                for id in ids {
                    let selector = html::selector_for_id(&id);
                    html = html::rename(&html, &selector, &name, None, false)
                        .map_err(|e| e.to_string())?;
                }
                html
            } else {
                let selector = ctx.selector()?;
                html::rename(&frame.html, &selector, &name, index, all).map_err(|e| e.to_string())?
            };
            frame.html = updated;
            frame.revision += 1;
            host.save_frame(&frame)?;
            Ok(text(json!({"frameId": frame.id, "name": name})))
        }
        "delete_nodes" => {
            let mut frame = ctx.frame()?;
            ensure_unlocked(&frame)?;
            let index = ctx.opt_u64("index").map(|i| i as usize);
            let all = ctx.opt_bool("all").unwrap_or(false);
            let updated = if let Some(ids) = ctx.opt_str_vec("nodeIds") {
                if ids.is_empty() {
                    return Err("`nodeIds` must not be empty".into());
                }
                let mut html = frame.html.clone();
                for id in ids {
                    let selector = html::selector_for_id(&id);
                    html = html::delete(&html, &selector, None, false)
                        .map_err(|e| e.to_string())?;
                }
                html
            } else {
                let selector = ctx.selector()?;
                html::delete(&frame.html, &selector, index, all).map_err(|e| e.to_string())?
            };
            host.store
                .push_version(&frame, Some("before delete_nodes"), 30)
                .map_err(err)?;
            frame.html = updated;
            frame.revision += 1;
            frame.approval = Approval::Draft;
            host.save_frame(&frame)?;
            Ok(text(json!({"frameId": frame.id, "deleted": "nodes"})))
        }
        "get_computed_styles" => {
            let frame = ctx.frame()?;
            let selector = ctx.selector()?;
            let styles =
                html::computed_styles(&frame.html, &selector).map_err(|e| e.to_string())?;
            Ok(text(json!(styles)))
        }
        "get_jsx" => {
            let frame = ctx.frame()?;
            Ok(vec![
                json!({"type": "text", "text": html::to_jsx(&frame.html)}),
            ])
        }
        "get_tokens" => {
            let canvas = ctx.canvas()?;
            let format = ctx.opt_str("format").unwrap_or_else(|| "json".into());
            let themes = canvas.theme_names();
            match format.as_str() {
                "json" => Ok(text(core_tokens::tokens_json(&canvas.tokens))),
                "css" => Ok(vec![
                    json!({"type": "text", "text": core_tokens::tokens_css_all(&canvas.tokens, &themes)}),
                ]),
                "tailwind" => Ok(vec![
                    json!({"type": "text", "text": core_tokens::tokens_tailwind(&canvas.tokens, &themes[0])}),
                ]),
                "markdown" => Ok(vec![
                    json!({"type": "text", "text": core_tokens::tokens_markdown(&canvas.tokens, &themes)}),
                ]),
                other => Err(format!("unsupported token format `{other}`")),
            }
        }
        "create_tokens" => {
            let mut canvas = ctx.canvas()?;
            let incoming = parse_tokens(&ctx.args["tokens"])?;
            if incoming.is_empty() {
                return Err("`tokens` is empty".into());
            }
            core_tokens::merge_tokens(&mut canvas.tokens, incoming);
            host.store.save_canvas(&canvas).map_err(err)?;
            host.emit(
                &canvas.id,
                json!({"type": "canvas.updated", "canvas": canvas}),
            );
            Ok(text(json!(canvas.tokens)))
        }
        "set_tokens" => {
            let mut canvas = ctx.canvas()?;
            if ctx.opt_bool("replace").unwrap_or(false) {
                canvas.tokens.clear();
            }
            if let Some(tokens) = ctx.args.get("tokens") {
                core_tokens::merge_tokens(&mut canvas.tokens, parse_tokens(tokens)?);
            }
            if let Some(delete) = ctx.args.get("delete").and_then(Value::as_array) {
                let names: Vec<String> = delete
                    .iter()
                    .filter_map(Value::as_str)
                    .map(core_tokens::normalize_name)
                    .collect();
                canvas.tokens.retain(|t| !names.contains(&t.name));
            }
            host.store.save_canvas(&canvas).map_err(err)?;
            host.emit(
                &canvas.id,
                json!({"type": "canvas.updated", "canvas": canvas}),
            );
            Ok(text(json!(canvas.tokens)))
        }
        "set_token_themes" => {
            let mut canvas = ctx.canvas()?;
            let themes: Vec<String> = ctx.args["themes"]
                .as_array()
                .ok_or("`themes` must be an array")?
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            if themes.is_empty() {
                return Err("a canvas needs at least one token theme".into());
            }
            canvas.token_themes = themes;
            host.store.save_canvas(&canvas).map_err(err)?;
            host.emit(
                &canvas.id,
                json!({"type": "canvas.updated", "canvas": canvas}),
            );
            Ok(text(json!(canvas.token_themes)))
        }
        "get_comments" => {
            let canvas = ctx.canvas()?;
            let frame_id = ctx.opt_str("frameId");
            let comments: Vec<Comment> = host
                .store
                .comments(&canvas.id)
                .map_err(err)?
                .into_iter()
                .filter(|c| {
                    frame_id
                        .as_deref()
                        .map(|f| c.frame_id.as_deref() == Some(f))
                        .unwrap_or(true)
                })
                .collect();
            Ok(text(json!(comments)))
        }
        "reply_to_comment" => {
            let id = ctx.req_str("commentId")?;
            let mut comment = host
                .store
                .get_comment(&id)
                .map_err(err)?
                .ok_or_else(|| format!("no comment `{id}`"))?;
            comment.replies.push(Reply {
                id: new_id(),
                author: ctx.opt_str("author").unwrap_or_else(|| "Agent".into()),
                body: ctx.req_str("body")?,
                created_at: now_ms(),
            });
            host.store.save_comment(&comment).map_err(err)?;
            host.emit(
                &comment.canvas_id,
                json!({"type": "comment.updated", "comment": comment}),
            );
            Ok(text(json!(comment)))
        }
        "resolve_comment" => {
            let id = ctx.req_str("commentId")?;
            let mut comment = host
                .store
                .get_comment(&id)
                .map_err(err)?
                .ok_or_else(|| format!("no comment `{id}`"))?;
            comment.resolved = ctx.opt_bool("resolved").unwrap_or(true);
            host.store.save_comment(&comment).map_err(err)?;
            host.emit(
                &comment.canvas_id,
                json!({"type": "comment.updated", "comment": comment}),
            );
            Ok(text(
                json!({"id": comment.id, "resolved": comment.resolved}),
            ))
        }
        "get_guidelines" => {
            let canvas = ctx.canvas()?;
            Ok(text(json!(canvas.guidelines)))
        }
        "set_guidelines" => {
            let mut canvas = ctx.canvas()?;
            let name = ctx.req_str("name")?;
            let body = ctx.req_str("body")?;
            match canvas.guidelines.iter_mut().find(|g| g.name == name) {
                Some(guide) => {
                    guide.body = body;
                    guide.updated_at = now_ms();
                }
                None => canvas.guidelines.push(Guideline {
                    id: new_id(),
                    name,
                    body,
                    updated_at: now_ms(),
                }),
            }
            host.store.save_canvas(&canvas).map_err(err)?;
            Ok(text(json!(canvas.guidelines)))
        }
        "save_decision" => {
            let mut canvas = ctx.canvas()?;
            let body = ctx.req_str("body")?;
            canvas.decisions.push(omapaper_core::Decision {
                id: new_id(),
                body,
                created_at: now_ms(),
            });
            host.store.save_canvas(&canvas).map_err(err)?;
            Ok(text(json!(canvas.decisions)))
        }
        "list_data_sources" => {
            let canvas = ctx.canvas()?;
            Ok(text(
                json!(host.store.data_sources(&canvas.id).map_err(err)?),
            ))
        }
        "pull_data" => {
            let canvas = ctx.canvas()?;
            let only = ctx.opt_str("sourceId");
            let mut values = std::collections::BTreeMap::new();
            for source in host.store.data_sources(&canvas.id).map_err(err)? {
                if only.as_deref().is_some_and(|only| only != source.id) {
                    continue;
                }
                let parsed = omapaper_core::data::read(&source.path, &host.data_dir, source.kind)?;
                values.extend(parsed);
                host.store.touch_data_source(&source.id).map_err(err)?;
            }
            if values.is_empty() {
                return Ok(text(json!({"frames": 0, "replaced": 0, "keys": 0})));
            }
            let mut frames = 0;
            let mut replaced = 0;
            for mut frame in host.store.frames(&canvas.id).map_err(err)? {
                let (html, count) = omapaper_core::data::apply(&frame.html, &values);
                if count == 0 || html == frame.html {
                    continue;
                }
                let _ = host
                    .store
                    .push_version(&frame, Some("before a data pull"), 30);
                frame.html = html;
                frame.revision += 1;
                host.store.save_frame(&frame).map_err(err)?;
                host.emit(
                    &frame.canvas_id,
                    json!({"type": "frame.updated", "canvasId": frame.canvas_id, "frame": frame}),
                );
                frames += 1;
                replaced += count;
            }
            Ok(text(json!({"frames": frames, "replaced": replaced, "keys": values.len()})))
        }
        "push_sync" => {
            let sync_id = ctx.req_str("syncId")?;
            let target = host
                .store
                .get_sync_target(&sync_id)
                .map_err(err)?
                .ok_or_else(|| format!("no sync target `{sync_id}`"))?;
            let frame = host
                .store
                .get_frame(&target.frame_id)
                .map_err(err)?
                .ok_or_else(|| format!("no frame `{}`", target.frame_id))?;
            let content = match target.kind {
                omapaper_core::model::SyncKind::Text => {
                    let found = html::find(&frame.html, &target.selector)
                        .map_err(|error| format!("{error:?}"))?;
                    let node = found.first().ok_or_else(|| {
                        format!("`{}` matched nothing", target.selector)
                    })?;
                    html::text_content(&html::serialize_node(&node.node))
                        .trim()
                        .to_string()
                }
                omapaper_core::model::SyncKind::Tokens => {
                    let canvas = host
                        .store
                        .get_canvas(&target.canvas_id)
                        .map_err(err)?
                        .ok_or_else(|| format!("no canvas `{}`", target.canvas_id))?;
                    core_tokens::tokens_css_all(&canvas.tokens, &canvas.token_themes)
                }
            };
            let path = omapaper_core::data::resolve(&target.path, &host.data_dir);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
            }
            std::fs::write(&path, &content)
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
            host.store.touch_sync_target(&sync_id).map_err(err)?;
            Ok(text(json!({
                "path": path.display().to_string(),
                "bytes": content.len()
            })))
        }
        "list_board_cards" => {
            let canvas = ctx.canvas()?;
            Ok(text(json!(host.store.cards(&canvas.id).map_err(err)?)))
        }
        "create_board_card" => {
            let canvas = ctx.canvas()?;
            let brief = ctx.req_str("brief")?;
            let role = match ctx.opt_str("role") {
                Some(value) => CardRole::parse(&value)
                    .ok_or_else(|| format!("unknown role `{value}`"))?,
                None => CardRole::default(),
            };
            let title = ctx
                .opt_str("title")
                .filter(|title| !title.trim().is_empty())
                .unwrap_or_else(|| {
                    brief
                        .lines()
                        .find(|line| !line.trim().is_empty())
                        .unwrap_or("Task")
                        .chars()
                        .take(64)
                        .collect()
                });
            let mut card = BoardCard::new(&canvas.id, title, brief, role);
            card.frame_id = ctx.opt_str("frameId");
            card.created_by = ctx.opt_str("author").unwrap_or_else(|| "agent".into());
            host.store.add_card(&card).map_err(err)?;
            host.touch_canvas(&canvas.id);
            let canvas_id = canvas.id.clone();
            host.emit(
                &canvas_id,
                json!({"type": "card.created", "canvasId": canvas_id, "card": card}),
            );
            Ok(text(json!(card)))
        }
        "update_board_card" => {
            let card_id = ctx.req_str("cardId")?;
            let mut card = host
                .store
                .get_card(&card_id)
                .map_err(err)?
                .ok_or_else(|| format!("no card `{card_id}`"))?;
            if let Some(status) = ctx.opt_str("status") {
                card.status = CardStatus::parse(&status)
                    .ok_or_else(|| format!("unknown status `{status}`"))?;
                if card.status != CardStatus::Failed {
                    card.error = None;
                }
            }
            if let Some(result) = ctx.opt_str("result") {
                card.result = Some(result);
            }
            if let Some(title) = ctx.opt_str("title") {
                card.title = title;
            }
            if let Some(brief) = ctx.opt_str("brief") {
                card.brief = brief;
            }
            host.store.save_card(&card).map_err(err)?;
            let canvas_id = card.canvas_id.clone();
            host.emit(
                &canvas_id,
                json!({"type": "card.updated", "canvasId": canvas_id, "card": card}),
            );
            Ok(text(json!(card)))
        }
        "finish_working" => {
            let canvas = ctx.canvas()?;
            let agent_name = ctx.req_str("agentName")?;
            let summary = ctx.opt_str("summary").unwrap_or_default();
            host.presence.set(
                &canvas.id,
                Peer {
                    id: format!("agent-{agent_name}"),
                    name: agent_name.clone(),
                    kind: PeerKind::Agent,
                    color: color_for(&agent_name),
                    frame_id: None,
                    x: 0.0,
                    y: 0.0,
                    status: None,
                    selection: Vec::new(),
                    updated_at: now_ms(),
                },
            );
            Ok(text(
                json!({"ok": true, "agent": agent_name, "summary": summary}),
            ))
        }
        other => Err(format!("unknown tool `{other}`")),
    }
}

// ---- context helpers ------------------------------------------------------

struct Ctx<'a> {
    host: &'a Host,
    args: &'a Value,
}

impl Ctx<'_> {
    fn req_str(&self, key: &str) -> Result<String, String> {
        self.args
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("`{key}` is required"))
    }

    fn opt_str_vec(&self, key: &str) -> Option<Vec<String>> {
        self.args
            .get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            })
    }

    fn opt_str(&self, key: &str) -> Option<String> {
        self.args
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    fn opt_f64(&self, key: &str) -> Option<f64> {
        self.args.get(key).and_then(Value::as_f64)
    }

    fn opt_u64(&self, key: &str) -> Option<u64> {
        self.args.get(key).and_then(Value::as_u64)
    }

    fn opt_bool(&self, key: &str) -> Option<bool> {
        self.args.get(key).and_then(Value::as_bool)
    }

    fn canvas(&self) -> Result<Canvas, String> {
        self.host
            .resolve_canvas(self.opt_str("canvasId").as_deref())
    }

    fn frame(&self) -> Result<Frame, String> {
        let id = self.req_str("frameId")?;
        self.host
            .store
            .get_frame(&id)
            .map_err(err)?
            .ok_or_else(|| format!("no frame `{id}`"))
    }

    /// Selector for node tools: `nodeId` wins, then `selector`.
    fn selector(&self) -> Result<String, String> {
        match (self.opt_str("nodeId"), self.opt_str("selector")) {
            (Some(id), _) => Ok(html::selector_for_id(&id)),
            (None, Some(selector)) => Ok(selector),
            (None, None) => Err("pass `nodeId` (a data-op-id) or `selector`".into()),
        }
    }

    fn style_map(&self, key: &str) -> Result<BTreeMap<String, String>, String> {
        let value = self
            .args
            .get(key)
            .and_then(Value::as_object)
            .ok_or_else(|| format!("`{key}` must be an object"))?;
        let mut map = BTreeMap::new();
        for (k, v) in value {
            let text = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            map.insert(k.clone(), text);
        }
        Ok(map)
    }
}

fn text(value: Value) -> Vec<Value> {
    vec![json!({"type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default()})]
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn ensure_unlocked(frame: &Frame) -> Result<(), String> {
    if frame.locked {
        Err(format!(
            "frame {} is locked; unlock it with update_frame",
            frame.id
        ))
    } else {
        Ok(())
    }
}

fn approval_key(approval: Approval) -> &'static str {
    match approval {
        Approval::Draft => "draft",
        Approval::ReadyForDev => "ready_for_dev",
        Approval::NeedsReview => "needs_review",
    }
}

fn parse_approval(value: &str) -> Result<Approval, String> {
    match value {
        "draft" => Ok(Approval::Draft),
        "needs_review" => Ok(Approval::NeedsReview),
        "ready_for_dev" => Ok(Approval::ReadyForDev),
        other => Err(format!("unknown approval `{other}`")),
    }
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or("").trim().to_string()
}

fn slug(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn parse_tokens(value: &Value) -> Result<Vec<Token>, String> {
    let array = value.as_array().ok_or("`tokens` must be an array")?;
    let mut tokens = Vec::new();
    for entry in array {
        let name = entry
            .get("name")
            .and_then(Value::as_str)
            .ok_or("every token needs a `name`")?;
        let kind = match entry.get("kind").and_then(Value::as_str) {
            Some("spacing") => TokenKind::Spacing,
            Some("radius") => TokenKind::Radius,
            Some("typography") => TokenKind::Typography,
            Some("shadow") => TokenKind::Shadow,
            Some("other") => TokenKind::Other,
            _ => TokenKind::Color,
        };
        let description = entry
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let values = entry
            .get("values")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("token `{name}` needs `values`"))?;
        let mut map = BTreeMap::new();
        for (theme, value) in values {
            let text = value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_f64().map(|n| n.to_string()))
                .ok_or_else(|| format!("token `{name}` has a non-string value"))?;
            map.insert(theme.clone(), text);
        }
        tokens.push(core_tokens::make_token(name, kind, description, map));
    }
    Ok(tokens)
}
