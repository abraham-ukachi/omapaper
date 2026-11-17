//! Plugins: one JSON manifest in `$DATA_DIR/plugins`, an optional command the
//! daemon may run. Execution is local and explicit — a manifest with no
//! `command` is documentation only, and the daemon never fetches code.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What a manifest file may declare.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub hooks: Vec<String>,
    /// The program and its arguments. Empty means manifest-only.
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub docs: Option<String>,
}

impl PluginManifest {
    /// The manifest's name, falling back to the file stem.
    pub fn display_name(&self, path: &Path) -> String {
        if !self.name.trim().is_empty() {
            return self.name.trim().to_string();
        }
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("plugin")
            .to_string()
    }

    pub fn runnable(&self) -> bool {
        !self.command.is_empty()
    }
}

/// A manifest and where it came from.
#[derive(Debug, Clone)]
pub struct Plugin {
    pub path: PathBuf,
    pub manifest: PluginManifest,
}

impl Plugin {
    pub fn name(&self) -> String {
        self.manifest.display_name(&self.path)
    }
}

/// Reads every `*.json` manifest in `dir`, sorted by name.
pub fn list(dir: &Path) -> Vec<Plugin> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<PluginManifest>(&text) else {
            continue;
        };
        out.push(Plugin { path, manifest });
    }
    out.sort_by_key(|plugin| plugin.name());
    out
}

/// Finds a plugin by manifest name or file stem.
pub fn find(dir: &Path, name: &str) -> Option<Plugin> {
    list(dir).into_iter().find(|plugin| {
        plugin.name() == name
            || plugin
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem == name)
                .unwrap_or(false)
    })
}

/// What a plugin answered.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRun {
    pub name: String,
    pub exit_code: i32,
    /// Parsed JSON when the plugin printed JSON, the text otherwise.
    pub output: Value,
    pub stderr: String,
}

/// Runs a plugin's command with `input` on stdin and reads its stdout.
pub fn run(dir: &Path, name: &str, input: &Value) -> Result<PluginRun, String> {
    let plugin = find(dir, name).ok_or_else(|| format!("no plugin `{name}`"))?;
    if !plugin.manifest.runnable() {
        return Err(format!(
            "`{}` is a manifest only — add a `command` array to let it run",
            plugin.name()
        ));
    }
    let program = &plugin.manifest.command[0];
    let mut child = Command::new(program)
        .args(&plugin.manifest.command[1..])
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot run `{program}`: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.to_string().as_bytes())
            .map_err(|error| format!("cannot send the plugin input: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("the plugin failed: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parsed = serde_json::from_str::<Value>(&stdout).unwrap_or_else(|_| json!(stdout));
    let run = PluginRun {
        name: plugin.name(),
        exit_code: output.status.code().unwrap_or(-1),
        output: parsed,
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    };
    if !output.status.success() {
        return Err(format!(
            "`{}` exited with {}{}",
            run.name,
            run.exit_code,
            if run.stderr.is_empty() {
                String::new()
            } else {
                format!(": {}", run.stderr)
            }
        ));
    }
    Ok(run)
}

/// The manifest shape `/api/plugins` returns.
pub fn describe(dir: &Path) -> Vec<Value> {
    list(dir)
        .into_iter()
        .map(|plugin| {
            json!({
                "name": plugin.name(),
                "version": plugin.manifest.version,
                "description": plugin.manifest.description,
                "hooks": plugin.manifest.hooks,
                "docs": plugin.manifest.docs,
                "runnable": plugin.manifest.runnable(),
                "file": plugin.path.file_name().map(|name| name.to_string_lossy().to_string()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("omapaper-plugins-{label}-{}", crate::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn manifests_are_listed_and_runnable_only_with_a_command() {
        let dir = temp_dir("list");
        std::fs::write(
            dir.join("docs.json"),
            r#"{"name":"docs","version":"0.1.0","hooks":["canvas.open"]}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("echo.json"),
            r#"{"name":"echo","command":["cat"]}"#,
        )
        .unwrap();
        let plugins = list(&dir);
        assert_eq!(plugins.len(), 2);
        assert!(!plugins[0].manifest.runnable() || !plugins[1].manifest.runnable());
        assert_eq!(describe(&dir).len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_plugin_echoes_its_input_as_json() {
        let dir = temp_dir("run");
        std::fs::write(
            dir.join("echo.json"),
            r#"{"name":"echo","command":["cat"]}"#,
        )
        .unwrap();
        let result = run(&dir, "echo", &json!({"hello": "world"})).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output["hello"], "world");
        assert!(run(&dir, "missing", &json!({})).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
