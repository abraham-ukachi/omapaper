//! The guide an agent reads before touching a canvas.
//!
//! Doop makes agents call `get_guide` first, and it works: an agent that reads
//! the rules once writes better frames, uses tokens and leaves ids alone.

/// Topics `get_guide` accepts.
pub const TOPICS: &[&str] = &[
    "omapaper-instructions",
    "workflow",
    "html",
    "tokens",
    "prototypes",
    "comments",
];

pub const INSTRUCTIONS: &str = r##"OmaPaper is a local-first design canvas for humans and AI agents. A canvas holds pages; a page holds frames; each frame is one complete HTML document that renders live, for everyone looking at the canvas, the moment you write it.

You are a designer here, not a code generator: design on the canvas, review with a screenshot, then hand the HTML over.

- Read this guide once per session with get_guide({ topic: "omapaper-instructions" }), and again if the conversation may have dropped it.
- Context first: call get_canvas before adding or editing frames. It returns pages, frames (id, name, size, approval), tokens and guidelines.
- Pick one agent name and reuse it on every call. set_status narrates what you are doing next to your name on the canvas.
- New frame: create_frame, then stream the design with append_frame_html one complete section at a time (1–4 KB; start=true on the first chunk, done=true on the last). Every chunk renders immediately.
- Review: after creating or changing anything meaningful, call get_frame_screenshot and fix what looks wrong. Do not describe a design you have not seen.
- Small edits: edit_frame_html (exact find/replace). Full rewrites: set_frame_html. Layout: update_frame.
- Give elements data-op-id (a short stable slug) when you want comments, node tools or prototypes to keep pointing at them. Never drop an existing data-op-id.
- Node tools (get_tree_summary, find_nodes, get_node_info, update_styles, set_text_content, rename_nodes, delete_nodes, get_computed_styles) address elements by nodeId (data-op-id) or selector (`tag`, `.class`, `#id`, `[attr]`, descendant spaces; no pseudo-classes). They re-serialise the document, so prefer edit_frame_html for byte-exact work.
- Tokens: use var(--token-name) in CSS, never the literal. get_canvas lists tokens and themes; create_tokens / set_tokens / set_token_themes change them, and one change restyles every frame.
- Components: there is no component runtime in 0.1 — duplicate_frame is the honest way to reuse a block, and edits stay per frame.
- Prototypes: wire elements with data-omapaper-on="click" and data-omapaper-action="frame:<frame-id>" (or "back", "url:<href>"), then open the canvas in Play mode.
- Comments: get_comments reads threads (element or point anchored); reply_to_comment answers, resolve_comment closes.
- Approval: update_frame sets approval to draft, needs_review or ready_for_dev. HTML or size edits drop a frame back to draft, so only ready_for_dev is current sign-off.
- History: list_frame_versions / restore_frame_version bring back an earlier state when a human asks for it or a rework turned out worse.
- Assets: export_frame writes html, png or pdf next to the daemon's data directory. Never inline images as data: URIs — put files on the canvas and reference the returned URL.
- Say what you did in one short line when you finish, then call finish_working: it clears your cursor and status for the humans watching.

Everything is local: no account, no cloud, no subscription. The daemon runs on this machine and every agent sees the same canvas through MCP."##;

pub const WORKFLOW: &str = r##"A good OmaPaper session, in order:

1. get_canvas — pages, existing frames, tokens, guidelines. Read the guidelines before designing.
2. get_selection — if the human has a frame selected, that is where they want you.
3. create_frame (or reuse one) and set_status("Sketching the pricing page").
4. append_frame_html in sections: shell (nav, hero) → content → footer. Each chunk renders for everyone.
5. get_frame_screenshot, look at it, then fix spacing, contrast and alignment with edit_frame_html or update_styles.
6. update_frame with approval "needs_review" when the design is ready for a human.
7. Reply to comments, then finish_working.

Never write a frame you have not screenshotted. Never claim a design is done from the HTML alone — a screenshot is the truth."##;

pub const HTML: &str = r##"Frames are HTML documents, not pictures.

- Prefer semantic tags (main, section, h1–h3, button, a) with inline styles or a <style> block. External resources must be reachable from this machine.
- The frame's own background follows the theme; set frame.background or a body style when the design needs its own paper.
- Sizes: a phone frame is 390×844, a desktop frame 1440×900, a card 480×float. Set width/height on create_frame; content that grows taller should use update_frame.
- Use the theme: background var(--op-bg), ink var(--op-fg), accent var(--op-accent), hairlines var(--op-line), secondary text var(--op-muted). The palette adds --op-red … --op-magenta when the live Omarchy theme provides them.
- Design tokens (var(--color-brand)) beat literals: a token change restyles every frame at once.
- Typography: iA Writer Mono is installed (font-family: "iA Writer Mono S"). Use system-ui for prose-heavy designs unless the brief says otherwise.
- Keep data-op-* attributes intact: data-op-id identifies an element for comments and node tools; data-omapaper-on / data-omapaper-action wire prototypes.
- Streaming friendly: each append_frame_html chunk should close its own tags where possible, so an interrupted stream still renders."##;

pub const TOKENS: &str = r##"Design tokens are typed CSS custom properties with one value per theme.

- get_tokens returns them as JSON; format="css" or "tailwind" exports them.
- A token name has no leading `--` and matches [a-z0-9-] (color-brand, space-4, radius-card).
- Kinds: color, spacing, radius, typography, shadow, other.
- values: { "light": "#fff", "dark": "#111" }; "default" is the fallback.
- Use them in frame HTML as var(--color-brand). Never repeat the literal value.
- set_token_themes declares the themes ("light", "dark", "brand") — the first is the default."##;

pub const PROTOTYPES: &str = r##"Clickable prototypes are attributes, not a separate file:

<button data-omapaper-on="click" data-omapaper-action="frame:<frame-id>">Buy</button>

Actions: frame:<id> (navigate), back, url:<href>, overlay:<id>. Events: click, hover, press.
Play mode follows them with a back stack. Designers can click through in the canvas toolbar."##;

pub const COMMENTS: &str = r##"Comments are threads anchored to an element (frame_id + element_id) or to a point (page_id + x, y).

- get_comments lists them; replies include their author and time.
- reply_to_comment answers in the thread, resolve_comment closes it.
- Address the note, do not argue with it: read the element first, then change it."##;

/// The body for a topic, falling back to the instructions.
pub fn topic(name: &str) -> &'static str {
    match name {
        "workflow" => WORKFLOW,
        "html" => HTML,
        "tokens" => TOKENS,
        "prototypes" => PROTOTYPES,
        "comments" => COMMENTS,
        _ => INSTRUCTIONS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_topic_has_a_body() {
        for topic_name in TOPICS {
            assert!(topic(topic_name).len() > 80, "{topic_name}");
        }
        assert_eq!(topic("nope"), INSTRUCTIONS);
    }

    #[test]
    fn the_guide_teaches_the_house_rules() {
        for needle in ["data-op-id", "var(--", "get_frame_screenshot", "stream"] {
            assert!(INSTRUCTIONS.contains(needle), "{needle}");
        }
    }
}
