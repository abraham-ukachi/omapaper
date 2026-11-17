//! The built-in agent runner.
//!
//! OmaPaper has no account and no cloud, so the runner is deliberately small:
//! it reads a key from the environment, speaks the OpenAI-compatible chat
//! completions shape to whatever endpoint you point it at, and hands back the
//! HTML a frame should become. Local models work too — any server that answers
//! `POST /chat/completions` with `choices[0].message.content`.

use crate::{model::*, Canvas, Frame};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

/// Where the runner sends its prompts. Absent when no key is configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

impl AgentConfig {
    /// `OMAPAPER_AGENT_API_KEY` (or `OPENAI_API_KEY`) turns the runner on.
    /// `OMAPAPER_AGENT_BASE_URL` and `OMAPAPER_AGENT_MODEL` pick the endpoint
    /// and the model; the defaults are OpenAI's.
    pub fn from_env() -> Option<AgentConfig> {
        let key = std::env::var("OMAPAPER_AGENT_API_KEY")
            .ok()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())?;
        Some(AgentConfig {
            base_url: std::env::var("OMAPAPER_AGENT_BASE_URL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "https://api.openai.com/v1".into()),
            model: std::env::var("OMAPAPER_AGENT_MODEL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "gpt-4o-mini".into()),
            api_key: key,
        })
    }

    pub fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    /// A config with a placeholder key, for tests.
    pub fn sample() -> AgentConfig {
        AgentConfig {
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "llama3".into(),
            api_key: "test".into(),
        }
    }
}

/// The system prompt: the house rules plus the specialist's persona.
pub fn system_prompt(role: CardRole) -> String {
    format!(
        "{persona}\n\nYou work on OmaPaper, a local-first design canvas. Frames are complete HTML documents.\n\
         Answer with one complete HTML document and nothing else: start at <!doctype html> and end at </html>.\n\
         Use semantic tags, inline styles or a <style> block, and keep any data-op-id attributes that already exist.\n\
         Use the canvas design tokens (var(--token-name)) when they are listed. No external scripts, no build step,\n\
         no markdown commentary around the document.",
        persona = role.persona()
    )
}

/// Everything the runner knows about the task: the card, the canvas, the frame.
pub struct TaskContext<'a> {
    pub card: &'a BoardCard,
    pub canvas: &'a Canvas,
    pub frame: Option<&'a Frame>,
}

/// The user prompt: the brief plus the canvas it lands on.
pub fn user_prompt(ctx: &TaskContext<'_>) -> String {
    let mut prompt = String::new();
    prompt.push_str(&format!(
        "# Task: {}\n\n{}\n",
        ctx.card.title, ctx.card.brief
    ));
    if !ctx.canvas.guidelines.is_empty() {
        prompt.push_str("\n# Guidelines\n");
        for guideline in &ctx.canvas.guidelines {
            prompt.push_str(&format!("- {}: {}\n", guideline.name, guideline.body));
        }
    }
    if !ctx.canvas.decisions.is_empty() {
        prompt.push_str("\n# Decisions already made\n");
        for decision in ctx.canvas.decisions.iter().take(12) {
            prompt.push_str(&format!("- {}\n", decision.body));
        }
    }
    if !ctx.canvas.tokens.is_empty() {
        prompt.push_str("\n# Tokens\n");
        for token in ctx.canvas.tokens.iter().take(40) {
            prompt.push_str(&format!(
                "- --{}: {} ({})\n",
                token.name,
                token.default_value(),
                token.kind.label()
            ));
        }
    }
    match ctx.frame {
        Some(frame) => {
            prompt.push_str(&format!(
                "\n# Frame\n{} is {}×{}px. Current document:\n\n{}\n\nReturn the whole frame with the change applied.\n",
                frame.name, frame.width, frame.height, frame.html
            ));
        }
        None => {
            prompt.push_str(
                "\n# Frame\nReturn a new frame as a full <html> document. A phone frame is 390×844, a desktop 1440×900.\n",
            );
        }
    }
    prompt
}

/// The chat completions body for a prompt pair.
pub fn request_body(config: &AgentConfig, system: &str, user: &str) -> Value {
    json!({
        "model": config.model,
        "temperature": 0.6,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
    })
}

/// Reads `choices[0].message.content`, or explains what came back instead.
pub fn parse_completion(body: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| format!("the endpoint did not return JSON: {}", truncate(body, 240)))?;
    if let Some(message) = value.get("error").and_then(|error| {
        error
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| error.as_str())
    }) {
        return Err(format!("the endpoint refused the task: {message}"));
    }
    value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| format!("no content in the reply: {}", truncate(body, 240)))
}

/// Pulls the HTML document out of a model reply, fenced or bare.
pub fn extract_document(reply: &str) -> Option<String> {
    for fence in ["```html", "```HTML", "```"] {
        if let Some(start) = reply.find(fence) {
            let rest = &reply[start + fence.len()..];
            let rest = rest.strip_prefix('\r').unwrap_or(rest);
            let rest = rest.strip_prefix('\n').unwrap_or(rest);
            if let Some(end) = rest.find("```") {
                let doc = rest[..end].trim();
                if looks_like_document(doc) {
                    return Some(doc.to_string());
                }
            }
        }
    }
    if let Some(start) = reply
        .find("<!doctype")
        .or_else(|| reply.find("<!DOCTYPE"))
        .or_else(|| reply.find("<html"))
    {
        let doc = reply[start..].trim();
        if looks_like_document(doc) {
            return Some(doc.to_string());
        }
    }
    None
}

fn looks_like_document(text: &str) -> bool {
    let head: String = text
        .chars()
        .take(4096)
        .collect::<String>()
        .to_ascii_lowercase();
    head.contains("<body") || head.contains("<!doctype") || head.contains("<html")
}

fn truncate(text: &str, limit: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= limit {
        return trimmed.to_string();
    }
    let cut: String = trimmed.chars().take(limit).collect();
    format!("{cut}…")
}

/// Calls the endpoint and returns the reply. Blocking: the daemon wraps it in
/// `spawn_blocking`, the stdio MCP server simply calls it.
pub fn run(config: &AgentConfig, system: &str, user: &str) -> Result<String, String> {
    let body = request_body(config, system, user).to_string();
    let mut child = Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "300",
            "-X",
            "POST",
            &config.endpoint(),
            "-H",
            "content-type: application/json",
            "-H",
            &format!("authorization: Bearer {}", config.api_key),
            "-d",
            "@-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot run curl (is it installed?): {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(body.as_bytes())
            .map_err(|error| format!("cannot send the task: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("the request failed: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("the request failed: {}", truncate(&stderr, 240)));
    }
    parse_completion(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completions_bodies_carry_the_model_and_both_messages() {
        let body = request_body(&AgentConfig::sample(), "rules", "brief");
        assert_eq!(body["model"], "llama3");
        assert_eq!(body["messages"][0]["content"], "rules");
        assert_eq!(body["messages"][1]["content"], "brief");
        assert_eq!(
            AgentConfig::sample().endpoint(),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
    }

    #[test]
    fn completions_are_read_or_explained() {
        let ok = r#"{"choices":[{"message":{"content":"<html><body>hi</body></html>"}}]}"#;
        assert!(parse_completion(ok).unwrap().contains("<body>hi"));
        let refused = r#"{"error":{"message":"bad key"}}"#;
        assert!(parse_completion(refused).unwrap_err().contains("bad key"));
        assert!(parse_completion("nope")
            .unwrap_err()
            .contains("did not return JSON"));
    }

    #[test]
    fn documents_come_out_of_fences_or_whole_replies() {
        let fenced = "Sure!\n```html\n<!doctype html><html><body><h1>A</h1></body></html>\n```\n";
        assert_eq!(
            extract_document(fenced).unwrap(),
            "<!doctype html><html><body><h1>A</h1></body></html>"
        );
        let bare = "Here you go: <html><body><p>b</p></body></html> done.";
        assert!(extract_document(bare).unwrap().contains("<p>b</p>"));
        assert!(extract_document("no html here").is_none());
    }

    #[test]
    fn the_prompt_carries_the_house_rules() {
        let canvas = Canvas::new("Demo");
        let card = BoardCard::new(&canvas.id, "Hero", "Bigger headline", CardRole::Design);
        let prompt = user_prompt(&TaskContext {
            card: &card,
            canvas: &canvas,
            frame: None,
        });
        assert!(prompt.contains("Bigger headline"));
        assert!(prompt.contains("<html"));
        assert!(system_prompt(CardRole::Copy).contains("copywriter"));
    }
}
