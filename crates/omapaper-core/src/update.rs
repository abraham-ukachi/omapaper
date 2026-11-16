//! Check for a newer OmaPaper release.
//!
//! One HTTPS request to GitHub's releases API, nothing else: the daemon reports
//! what it found and the cockpit shows a line. No download runs on its own.

use serde::{Deserialize, Serialize};

/// What `/api/update` reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub url: String,
    pub update_available: bool,
    pub checked_at: i64,
}

/// Reads `tag_name` and `html_url` out of a GitHub release body.
pub fn parse_release(body: &str) -> Result<(String, String), String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "GitHub did not answer with JSON".to_string())?;
    if let Some(message) = value.get("message").and_then(|m| m.as_str()) {
        return Err(format!("GitHub said: {message}"));
    }
    let tag = value
        .get("tag_name")
        .and_then(|t| t.as_str())
        .filter(|tag| !tag.is_empty())
        .ok_or_else(|| "the release has no tag".to_string())?;
    let url = value
        .get("html_url")
        .and_then(|u| u.as_str())
        .unwrap_or("https://github.com/abraham-ukachi/omapaper/releases");
    Ok((tag.to_string(), url.to_string()))
}

/// True when `latest` is a later release than `current`. A plain release beats
/// the same number as a prerelease (`0.12.0` > `0.12.0-beta.2`).
pub fn is_newer(current: &str, latest: &str) -> bool {
    let (current_core, current_pre) = split_version(current);
    let (latest_core, latest_pre) = split_version(latest);
    let width = current_core.len().max(latest_core.len());
    for index in 0..width {
        let a = current_core.get(index).copied().unwrap_or(0);
        let b = latest_core.get(index).copied().unwrap_or(0);
        if b != a {
            return b > a;
        }
    }
    current_pre && !latest_pre
}

fn split_version(version: &str) -> (Vec<u64>, bool) {
    let version = version.trim().trim_start_matches(['v', 'V']);
    let (core, pre) = match version.split_once('-') {
        Some((core, _)) => (core, true),
        None => (version, false),
    };
    let numbers = core
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect();
    (numbers, pre)
}

/// Asks GitHub for the latest release. Blocking (curl); wrap it in a thread.
pub fn check(repo: &str, current: &str) -> Result<UpdateInfo, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let output = std::process::Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "12",
            "-H",
            "accept: application/vnd.github+json",
            "-H",
            "user-agent: omapaper",
            &url,
        ])
        .output()
        .map_err(|error| format!("cannot run curl: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "the update check failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let (latest, url) = parse_release(&String::from_utf8_lossy(&output.stdout))?;
    Ok(UpdateInfo {
        current: current.to_string(),
        update_available: is_newer(current, &latest),
        latest,
        url,
        checked_at: crate::now_ms(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_body_gives_a_tag_and_a_link() {
        let (tag, url) = parse_release(
            r#"{"tag_name":"v0.12.0-beta.1","html_url":"https://github.com/x/y/releases/tag/v0.12"}"#,
        )
        .unwrap();
        assert_eq!(tag, "v0.12.0-beta.1");
        assert!(url.ends_with("v0.12"));
        assert!(parse_release(r#"{"message":"Not Found"}"#)
            .unwrap_err()
            .contains("Not Found"));
    }

    #[test]
    fn newer_releases_are_spotted() {
        assert!(is_newer("0.9.0-beta.1", "v0.10.0"));
        assert!(is_newer("0.12.0-beta.2", "v0.12.0"));
        assert!(!is_newer("0.12.0", "v0.12.0"));
        assert!(!is_newer("0.12.0", "v0.11.9"));
        assert!(is_newer("1.0.0", "1.0.1"));
    }
}
