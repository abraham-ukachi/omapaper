//! Real data, local files only.
//!
//! A canvas can point at a JSON, CSV or `.env` file on this machine and pull
//! its values into a frame: every `{{dotted.key}}` in the frame's HTML is
//! replaced with the file's value. No database driver, no network — the file
//! is the source of truth and the frame keeps the last pull until the next one.

use crate::model::SourceKind;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Absolute paths are used as-is; relative ones live under `base`.
pub fn resolve(path: &str, base: &Path) -> PathBuf {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base.join(candidate)
    }
}

/// Reads a source file and flattens it into dotted keys.
pub fn read(path: &str, base: &Path, kind: SourceKind) -> Result<BTreeMap<String, String>, String> {
    let resolved = resolve(path, base);
    let text = std::fs::read_to_string(&resolved)
        .map_err(|error| format!("cannot read {}: {error}", resolved.display()))?;
    parse(kind, &text)
}

/// Parses source text into dotted keys.
pub fn parse(kind: SourceKind, text: &str) -> Result<BTreeMap<String, String>, String> {
    match kind {
        SourceKind::Json => {
            let value: Value =
                serde_json::from_str(text).map_err(|error| format!("invalid JSON: {error}"))?;
            let mut out = BTreeMap::new();
            flatten(&value, "", &mut out);
            Ok(out)
        }
        SourceKind::Csv => parse_csv(text),
        SourceKind::Env => {
            let mut out = BTreeMap::new();
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let line = line.strip_prefix("export ").unwrap_or(line);
                if let Some((key, value)) = line.split_once('=') {
                    let value = value.trim().trim_matches(['"', '\'']).to_string();
                    out.insert(key.trim().to_string(), value);
                }
            }
            Ok(out)
        }
    }
}

fn flatten(value: &Value, prefix: &str, out: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(value, &path, out);
            }
        }
        Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                flatten(value, &format!("{prefix}.{index}"), out);
            }
        }
        Value::String(text) => {
            out.insert(prefix.to_string(), text.clone());
        }
        Value::Null => {}
        other => {
            out.insert(prefix.to_string(), other.to_string());
        }
    }
}

/// The first row is the header. One data row gives flat keys; more give
/// `0.headline`, `1.headline`, …
fn parse_csv(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut rows: Vec<Vec<String>> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(split_csv)
        .collect();
    if rows.is_empty() {
        return Ok(BTreeMap::new());
    }
    let headers = rows.remove(0);
    let mut out = BTreeMap::new();
    let single = rows.len() == 1;
    for (index, row) in rows.iter().enumerate() {
        for (column, header) in headers.iter().enumerate() {
            let Some(value) = row.get(column) else {
                continue;
            };
            let key = if single {
                header.trim().to_string()
            } else {
                format!("{index}.{}", header.trim())
            };
            out.insert(key, value.clone());
        }
    }
    Ok(out)
}

fn split_csv(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut field)),
            other => field.push(other),
        }
    }
    fields.push(field);
    fields
}

/// Replaces every `{{key}}` that the source knows. Unknown keys are left
/// alone, so a frame can mix pulled data with a literal that looks similar.
/// Returns the new HTML and how many values were written.
pub fn apply(html: &str, values: &BTreeMap<String, String>) -> (String, usize) {
    let mut out = String::with_capacity(html.len());
    let mut replaced = 0;
    let mut rest = html;
    while let Some(start) = rest.find("{{") {
        let Some(end) = rest[start + 2..].find("}}") else {
            break;
        };
        let key = rest[start + 2..start + 2 + end].trim();
        match values.get(key) {
            Some(value) => {
                out.push_str(&rest[..start]);
                out.push_str(value);
                replaced += 1;
            }
            None => out.push_str(&rest[..start + 2 + end + 2]),
        }
        rest = &rest[start + 2 + end + 2..];
    }
    out.push_str(rest);
    (out, replaced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_flattens_into_dotted_keys() {
        let values = parse(
            SourceKind::Json,
            r#"{"hero":{"headline":"Ship it","price":9},"items":["a","b"]}"#,
        )
        .unwrap();
        assert_eq!(values["hero.headline"], "Ship it");
        assert_eq!(values["hero.price"], "9");
        assert_eq!(values["items.1"], "b");
    }

    #[test]
    fn csv_reads_one_row_flat_and_many_rows_indexed() {
        let single = parse(SourceKind::Csv, "name,price\nPro,12\n").unwrap();
        assert_eq!(single["name"], "Pro");
        let many = parse(SourceKind::Csv, "name,price\nPro,12\nFree,0\n").unwrap();
        assert_eq!(many["1.name"], "Free");
    }

    #[test]
    fn env_ignores_comments_and_quotes() {
        let values = parse(
            SourceKind::Env,
            "# comment\nAPP_NAME=\"OmaPaper\"\nexport PORT=4310\n",
        )
        .unwrap();
        assert_eq!(values["APP_NAME"], "OmaPaper");
        assert_eq!(values["PORT"], "4310");
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn apply_replaces_known_keys_only() {
        let values = parse(SourceKind::Json, r#"{"a":"one"}"#).unwrap();
        let (html, count) = apply("<h1>{{a}}</h1><p>{{missing}}</p><p>{{ a }}</p>", &values);
        assert_eq!(html, "<h1>one</h1><p>{{missing}}</p><p>one</p>");
        assert_eq!(count, 2);
    }

    #[test]
    fn paths_resolve_against_the_data_dir() {
        let base = Path::new("/tmp/omapaper");
        assert_eq!(resolve("data/x.json", base), base.join("data/x.json"));
        assert_eq!(resolve("/abs/x.json", base), Path::new("/abs/x.json"));
    }
}
