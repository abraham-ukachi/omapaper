//! Design tokens: typed CSS custom properties, one value per theme.
//!
//! Frames use them as `var(--color-brand)`, never as literals, so changing a
//! token restyles every frame of the canvas at once.

use crate::model::{Token, TokenKind};
use std::collections::BTreeMap;

/// Normalises a token name: no leading `--`, lower-case, dashes.
pub fn normalize_name(name: &str) -> String {
    name.trim()
        .trim_start_matches("--")
        .trim()
        .to_lowercase()
        .replace([' ', '_'], "-")
}

/// Validates a token name (`a-z0-9-`, starts with a letter).
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Builds a token from loose JSON values, normalising the name.
pub fn make_token(
    name: &str,
    kind: TokenKind,
    description: &str,
    values: BTreeMap<String, String>,
) -> Token {
    Token {
        name: normalize_name(name),
        kind,
        description: description.to_string(),
        values,
    }
}

/// `:root { --name: value }` for the given theme (defaults fall back).
pub fn tokens_css(tokens: &[Token], theme: &str) -> String {
    if tokens.is_empty() {
        return String::new();
    }
    let mut css = String::from(":root{\n");
    for token in tokens {
        css.push_str(&format!(
            "  --{}: {};\n",
            token.name,
            token.value_for(theme).unwrap_or("")
        ));
    }
    css.push_str("}\n");
    css
}

/// Every theme, one block each, for the canvas viewer.
pub fn tokens_css_all(tokens: &[Token], themes: &[String]) -> String {
    let mut css = String::new();
    for (index, theme) in themes.iter().enumerate() {
        let selector = if index == 0 {
            format!("[data-omapaper-theme=\"{theme}\"], :root")
        } else {
            format!("[data-omapaper-theme=\"{theme}\"]")
        };
        css.push_str(&format!("{selector}{{\n"));
        for token in tokens {
            if let Some(value) = token.value_for(theme) {
                css.push_str(&format!("  --{}: {};\n", token.name, value));
            }
        }
        css.push_str("}\n");
    }
    css
}

/// JSON: `{ "color-brand": { "light": "#…", "dark": "#…" }, … }`.
pub fn tokens_json(tokens: &[Token]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for token in tokens {
        let values: serde_json::Map<String, serde_json::Value> = token
            .values
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        map.insert(token.name.clone(), serde_json::Value::Object(values));
    }
    serde_json::Value::Object(map)
}

/// A Tailwind v4 `@theme` block.
pub fn tokens_tailwind(tokens: &[Token], theme: &str) -> String {
    if tokens.is_empty() {
        return String::new();
    }
    let mut css = String::from("@theme {\n");
    for token in tokens {
        css.push_str(&format!(
            "  --{}: {};\n",
            token.name,
            token.value_for(theme).unwrap_or("")
        ));
    }
    css.push_str("}\n");
    css
}

/// Markdown table of every token and its values.
pub fn tokens_markdown(tokens: &[Token], themes: &[String]) -> String {
    if tokens.is_empty() {
        return "No design tokens yet.\n".to_string();
    }
    let mut md = String::from("| Token | Type |");
    for theme in themes {
        md.push_str(&format!(" {theme} |"));
    }
    md.push_str(" Notes |\n|---|---|");
    for _ in themes {
        md.push_str("---|");
    }
    md.push_str("---|\n");
    for token in tokens {
        md.push_str(&format!("| `--{}` | {:?} |", token.name, token.kind));
        for theme in themes {
            md.push_str(&format!(" {} |", token.value_for(theme).unwrap_or("—")));
        }
        md.push_str(&format!(" {} |\n", token.description));
    }
    md
}

/// Merges incoming tokens into the canvas: same name replaces, new appends.
pub fn merge_tokens(existing: &mut Vec<Token>, incoming: Vec<Token>) {
    for token in incoming {
        if let Some(slot) = existing.iter_mut().find(|t| t.name == token.name) {
            *slot = token;
        } else {
            existing.push(token);
        }
    }
    existing.sort_by(|a, b| a.name.cmp(&b.name));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token() -> Token {
        let mut values = BTreeMap::new();
        values.insert("light".into(), "#111".into());
        values.insert("dark".into(), "#eee".into());
        make_token("color-ink", TokenKind::Color, "Body ink", values)
    }

    #[test]
    fn names_are_normalised() {
        assert_eq!(normalize_name("--Color Brand"), "color-brand");
        assert!(valid_name("color-brand-2"));
        assert!(!valid_name("2brand"));
        assert!(!valid_name("colour brand"));
    }

    #[test]
    fn css_blocks_default_to_the_theme() {
        let css = tokens_css(&[token()], "dark");
        assert!(css.contains("--color-ink: #eee"));
        let all = tokens_css_all(&[token()], &["light".into(), "dark".into()]);
        assert!(all.contains(":root"));
        assert!(all.contains("[data-omapaper-theme=\"dark\"]"));
    }

    #[test]
    fn exports_are_usable() {
        let tokens = vec![token()];
        assert!(tokens_tailwind(&tokens, "light").starts_with("@theme"));
        assert!(tokens_markdown(&tokens, &["light".into()]).contains("--color-ink"));
        assert_eq!(tokens_json(&tokens)["color-ink"]["dark"], "#eee");
    }

    #[test]
    fn merging_replaces_and_sorts() {
        let mut existing = vec![token()];
        let mut values = BTreeMap::new();
        values.insert("default".into(), "8px".into());
        merge_tokens(
            &mut existing,
            vec![
                make_token("space-2", TokenKind::Spacing, "", values),
                make_token("color-ink", TokenKind::Color, "New", BTreeMap::new()),
            ],
        );
        assert_eq!(existing.len(), 2);
        assert_eq!(existing[0].name, "color-ink");
        assert_eq!(existing[0].description, "New");
    }
}
