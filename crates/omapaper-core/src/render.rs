//! Turning a stored frame into the document a browser (or exporter) renders.
//!
//! A frame keeps its own HTML untouched; the theme, design tokens and the
//! canvas runtime are injected at read time, so switching theme or editing a
//! token restyles every frame without rewriting one byte of it.

use crate::model::Frame;
use crate::theme::Theme;

/// Builds the document served at `/frames/<id>/html`.
pub fn frame_document(frame: &Frame, theme: &Theme, tokens_css: &str) -> String {
    let mut style = String::new();
    style.push_str(&theme.css_vars());
    style.push_str(tokens_css);
    style.push_str(
        "html,body{margin:0;padding:0;}\n\
         body{background:var(--op-frame-bg, transparent); color:var(--op-fg);}\n\
         *,*::before,*::after{box-sizing:border-box;}\n\
         a{color:var(--op-accent);}\n\
         ::selection{background:var(--op-accent); color:var(--op-ink-on-accent);}\n\
         [data-op-selected]{outline:2px solid var(--op-accent) !important; outline-offset:2px;}\n",
    );

    let mut head_extra = format!("<style id=\"omapaper-theme\">{style}</style>\n");
    if let Some(background) = &frame.background {
        head_extra.push_str(&format!(
            "<style>body{{background:{background};}}</style>\n"
        ));
    }

    let theme_attr = frame
        .theme
        .as_deref()
        .map(|theme| format!(" data-omapaper-theme=\"{}\"", escape(theme)))
        .unwrap_or_default();

    inject(&frame.html, &theme_attr, &head_extra)
}

fn inject(html: &str, theme_attr: &str, head_extra: &str) -> String {
    let mut out = html.to_string();
    if !theme_attr.is_empty() {
        if let Some(index) = html_tag_insert_index(&out) {
            out.insert_str(index, theme_attr);
        }
    }
    if let Some(index) = out.to_ascii_lowercase().find("</head>") {
        out.insert_str(index, head_extra);
        return out;
    }
    // A fragment: give it a head of its own.
    format!(
        "<!doctype html><html{theme_attr}><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">{head_extra}</head><body>{html}</body></html>"
    )
}

/// Where to insert attributes inside the opening `<html …>` tag, unless a
/// theme attribute is already there.
fn html_tag_insert_index(html: &str) -> Option<usize> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<html")?;
    let end = lower[start..].find('>')? + start;
    if lower[start..end].contains("data-omapaper-theme") {
        return None;
    }
    Some(end)
}

fn escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Frame;

    fn frame() -> Frame {
        let mut frame = Frame::new("c", "p", "Hero");
        frame.html =
            "<!doctype html><html><head><title>x</title></head><body><h1>Hi</h1></body></html>"
                .into();
        frame
    }

    #[test]
    fn injects_theme_and_tokens_before_head_close() {
        let theme = Theme::named("Ristretto");
        let doc = frame_document(&frame(), &theme, ":root{--color-brand:#f38d70;}");
        assert!(doc.contains("--op-bg"));
        assert!(doc.contains("--color-brand"));
        assert!(doc.contains("<h1>Hi</h1>"));
        let head_close = doc.find("</head>").unwrap();
        assert!(doc.find("--op-bg").unwrap() < head_close);
    }

    #[test]
    fn frame_background_and_theme_attribute_are_applied() {
        let mut frame = frame();
        frame.background = Some("#101010".into());
        frame.theme = Some("dark".into());
        let doc = frame_document(&frame, &Theme::named("Nord"), "");
        assert!(doc.contains("data-omapaper-theme=\"dark\""));
        assert!(doc.contains("body{background:#101010;}"));
    }

    #[test]
    fn fragments_get_a_document() {
        let mut frame = frame();
        frame.html = "<p>Raw</p>".into();
        let doc = frame_document(&frame, &Theme::named("Nord"), "");
        assert!(doc.starts_with("<!doctype html>"));
        assert!(doc.contains("<body><p>Raw</p></body>"));
    }
}
