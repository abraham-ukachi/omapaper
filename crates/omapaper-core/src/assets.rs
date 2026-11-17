//! Generated material going onto the canvas: SVG layers and the small amount
//! of HTML surgery that places them. Frames stay whole HTML documents, so an
//! inserted SVG is one more positioned block in the body.

/// Checks that a string really is an `<svg>` element.
pub fn validate_svg(svg: &str) -> Result<(), String> {
    let head: String = svg.trim_start().chars().take(64).collect();
    let lower = head.to_ascii_lowercase();
    if !lower.starts_with("<svg") {
        return Err("pass an <svg …> element".into());
    }
    if !svg.to_ascii_lowercase().contains("</svg>") {
        return Err("the SVG is not closed (missing </svg>)".into());
    }
    Ok(())
}

/// Wraps an SVG in a positioned layer so it can be placed like any shape.
pub fn svg_layer(
    svg: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    name: Option<&str>,
) -> Result<String, String> {
    validate_svg(svg)?;
    let label = name
        .map(|name| format!(" data-op-svg=\"{}\"", escape_attr(name)))
        .unwrap_or_default();
    let size = if width > 0.0 && height > 0.0 {
        format!("width:{width}px;height:{height}px;")
    } else {
        "width:auto;height:auto;".to_string()
    };
    Ok(format!(
        "<div{label} style=\"position:absolute;left:{x}px;top:{y}px;{size}pointer-events:none;\">{svg}</div>"
    ))
}

/// Inserts a block just before `</body>`; documents without one get it appended.
pub fn insert_before_body(html: &str, block: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if let Some(index) = lower.rfind("</body>") {
        let mut out = String::with_capacity(html.len() + block.len() + 1);
        out.push_str(html[..index].trim_end());
        out.push('\n');
        out.push_str(block);
        out.push('\n');
        out.push_str(&html[index..]);
        return out;
    }
    format!("{html}\n{block}\n")
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_svg_elements_are_accepted() {
        assert!(validate_svg("<svg viewBox=\"0 0 10 10\"><circle r=\"2\"/></svg>").is_ok());
        assert!(validate_svg("<div>no</div>").is_err());
        assert!(validate_svg("<svg><circle r=\"2\"/>").is_err());
    }

    #[test]
    fn layers_are_positioned_and_labelled() {
        let layer = svg_layer("<svg></svg>", 8.0, 12.0, 64.0, 64.0, Some("Logo")).expect("a layer");
        assert!(layer.contains("left:8px;top:12px"));
        assert!(layer.contains("width:64px;height:64px"));
        assert!(layer.contains("data-op-svg=\"Logo\""));
        let layer = svg_layer("<svg></svg>", 0.0, 0.0, 0.0, 0.0, None).unwrap();
        assert!(layer.contains("width:auto"));
    }

    #[test]
    fn blocks_land_before_the_body_closes() {
        let html = "<html><body><h1>Hi</h1></body></html>";
        let out = insert_before_body(html, "<p>new</p>");
        assert!(out.contains("<h1>Hi</h1>\n<p>new</p>\n</body>"));
        let out = insert_before_body("<div>fragment</div>", "<p>new</p>");
        assert!(out.ends_with("<p>new</p>\n"));
    }
}
