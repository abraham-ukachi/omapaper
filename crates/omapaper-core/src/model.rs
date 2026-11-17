//! The OmaPaper document model.
//!
//! A **Canvas** holds **Pages**; a page holds **Frames**, and a frame is one
//! complete HTML document that renders live for everybody looking at it. The
//! model is deliberately small: everything an agent can create is one of these
//! five records, plus comments and versions.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A short, URL-safe identifier (12 hex chars).
pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

/// Milliseconds since the Unix epoch — the only clock OmaPaper stores.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Who is doing something on a canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PeerKind {
    #[default]
    Human,
    Agent,
}

/// A live participant: a person's tab, the desktop app, or an agent mid-task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: PeerKind,
    /// `#rrggbb`, derived from the name so a peer keeps its colour.
    pub color: String,
    #[serde(default)]
    pub frame_id: Option<String>,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    /// One line, in the agent's own words ("Streaming the pricing section").
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub selection: Vec<String>,
    pub updated_at: i64,
}

/// One page of a canvas. Frames belong to exactly one page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub id: String,
    pub name: String,
    /// Optional page colour; `None` shows the theme paper.
    #[serde(default)]
    pub background: Option<String>,
}

impl Page {
    pub fn new(name: impl Into<String>) -> Page {
        Page {
            id: new_id(),
            name: name.into(),
            background: None,
        }
    }
}

/// A design token: one value per theme, exported as CSS custom properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    #[default]
    Color,
    Spacing,
    Radius,
    Typography,
    Shadow,
    Other,
}

impl TokenKind {
    pub fn label(self) -> &'static str {
        match self {
            TokenKind::Color => "color",
            TokenKind::Spacing => "spacing",
            TokenKind::Radius => "radius",
            TokenKind::Typography => "typography",
            TokenKind::Shadow => "shadow",
            TokenKind::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    /// Without the leading `--` (`color-brand`, not `--color-brand`).
    pub name: String,
    #[serde(default)]
    pub kind: TokenKind,
    #[serde(default)]
    pub description: String,
    /// theme name -> value; `default` is the fallback theme.
    pub values: BTreeMap<String, String>,
}

impl Token {
    pub fn value_for(&self, theme: &str) -> Option<&str> {
        self.values
            .get(theme)
            .or_else(|| self.values.get("default"))
            .map(String::as_str)
    }

    pub fn default_value(&self) -> &str {
        self.values
            .get("default")
            .or_else(|| self.values.values().next())
            .map(String::as_str)
            .unwrap_or("")
    }
}

/// A named style guide on a canvas (brand rules, a recipe to follow).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Guideline {
    pub id: String,
    pub name: String,
    pub body: String,
    pub updated_at: i64,
}

/// A design decision the canvas remembers ("more air in the hero").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub id: String,
    pub body: String,
    pub created_at: i64,
}

/// An outbound webhook registered on a canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Webhook {
    pub id: String,
    pub canvas_id: String,
    pub url: String,
    #[serde(default)]
    pub events: Vec<String>,
    pub created_at: i64,
}

/// A specialist an agent task is handed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CardRole {
    #[default]
    Design,
    Copy,
    Code,
    Review,
    Research,
}

impl CardRole {
    pub fn key(self) -> &'static str {
        match self {
            CardRole::Design => "design",
            CardRole::Copy => "copy",
            CardRole::Code => "code",
            CardRole::Review => "review",
            CardRole::Research => "research",
        }
    }

    pub fn parse(value: &str) -> Option<CardRole> {
        match value.trim().trim_start_matches('@') {
            "design" => Some(CardRole::Design),
            "copy" => Some(CardRole::Copy),
            "code" => Some(CardRole::Code),
            "review" => Some(CardRole::Review),
            "research" => Some(CardRole::Research),
            _ => None,
        }
    }

    /// The line the runner puts in the system prompt.
    pub fn persona(self) -> &'static str {
        match self {
            CardRole::Design => {
                "You are a product designer. You shape screens on the canvas: layout, hierarchy, spacing and type."
            }
            CardRole::Copy => {
                "You are a copywriter who works inside designs. You write short, plain, specific interface copy and never invent claims."
            }
            CardRole::Code => {
                "You are a front-end engineer. You write semantic HTML and CSS that mirrors the design, and you keep every data-op-id intact."
            }
            CardRole::Review => {
                "You are a design reviewer. You audit the work against the canvas guidelines and say what to change, in order."
            }
            CardRole::Research => {
                "You are a product researcher. You turn a brief into structure: sections, states, and the questions a design must answer."
            }
        }
    }
}

/// Where a board card stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CardStatus {
    #[default]
    Queued,
    Working,
    Done,
    Failed,
}

impl CardStatus {
    pub fn key(self) -> &'static str {
        match self {
            CardStatus::Queued => "queued",
            CardStatus::Working => "working",
            CardStatus::Done => "done",
            CardStatus::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<CardStatus> {
        match value {
            "queued" => Some(CardStatus::Queued),
            "working" => Some(CardStatus::Working),
            "done" => Some(CardStatus::Done),
            "failed" => Some(CardStatus::Failed),
            _ => None,
        }
    }
}

/// One task on a canvas board: a brief, the specialist it is for, and what
/// came back. Cards are queued by a human, an @mention or an automation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardCard {
    pub id: String,
    pub canvas_id: String,
    pub title: String,
    pub brief: String,
    #[serde(default)]
    pub role: CardRole,
    #[serde(default)]
    pub status: CardStatus,
    /// The frame the card works on, when it has one.
    #[serde(default)]
    pub frame_id: Option<String>,
    /// `You`, `@design`, `automation:Name` — who asked.
    #[serde(default)]
    pub created_by: String,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl BoardCard {
    pub fn new(
        canvas_id: &str,
        title: impl Into<String>,
        brief: impl Into<String>,
        role: CardRole,
    ) -> BoardCard {
        let now = now_ms();
        BoardCard {
            id: new_id(),
            canvas_id: canvas_id.into(),
            title: title.into(),
            brief: brief.into(),
            role,
            status: CardStatus::Queued,
            frame_id: None,
            created_by: String::new(),
            result: None,
            error: None,
            created_at: now,
            updated_at: now,
        }
    }
}

/// What makes an automation queue a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutomationTrigger {
    #[default]
    CommentCreated,
    FrameApproved,
    Schedule,
}

impl AutomationTrigger {
    pub fn key(self) -> &'static str {
        match self {
            AutomationTrigger::CommentCreated => "comment_created",
            AutomationTrigger::FrameApproved => "frame_approved",
            AutomationTrigger::Schedule => "schedule",
        }
    }

    pub fn parse(value: &str) -> Option<AutomationTrigger> {
        match value {
            "comment_created" => Some(AutomationTrigger::CommentCreated),
            "frame_approved" => Some(AutomationTrigger::FrameApproved),
            "schedule" => Some(AutomationTrigger::Schedule),
            _ => None,
        }
    }
}

/// A local workflow: when something happens (or every N minutes), queue a card
/// for a specialist. No server, no scheduler service — the daemon ticks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Automation {
    pub id: String,
    pub canvas_id: String,
    pub name: String,
    #[serde(default)]
    pub trigger: AutomationTrigger,
    /// For `schedule`: minutes between runs.
    #[serde(default)]
    pub interval_minutes: Option<i64>,
    pub brief: String,
    #[serde(default)]
    pub role: CardRole,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub last_run_at: Option<i64>,
    pub created_at: i64,
}

pub fn default_true() -> bool {
    true
}

impl Automation {
    pub fn new(canvas_id: &str, name: impl Into<String>, brief: impl Into<String>) -> Automation {
        Automation {
            id: new_id(),
            canvas_id: canvas_id.into(),
            name: name.into(),
            trigger: AutomationTrigger::default(),
            interval_minutes: None,
            brief: brief.into(),
            role: CardRole::default(),
            enabled: true,
            last_run_at: None,
            created_at: now_ms(),
        }
    }

    /// A scheduled automation is due once its interval has passed.
    pub fn is_due(&self, now: i64) -> bool {
        if !self.enabled || self.trigger != AutomationTrigger::Schedule {
            return false;
        }
        let Some(minutes) = self.interval_minutes.filter(|m| *m > 0) else {
            return false;
        };
        match self.last_run_at {
            None => true,
            Some(last) => now - last >= minutes * 60_000,
        }
    }
}

/// Where a data source reads from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    #[default]
    Json,
    Csv,
    Env,
}

impl SourceKind {
    pub fn key(self) -> &'static str {
        match self {
            SourceKind::Json => "json",
            SourceKind::Csv => "csv",
            SourceKind::Env => "env",
        }
    }

    pub fn parse(value: &str) -> Option<SourceKind> {
        match value {
            "json" => Some(SourceKind::Json),
            "csv" => Some(SourceKind::Csv),
            "env" => Some(SourceKind::Env),
            _ => None,
        }
    }
}

/// A local file a canvas can pull real content from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSource {
    pub id: String,
    pub canvas_id: String,
    pub name: String,
    #[serde(default)]
    pub kind: SourceKind,
    /// Absolute path, or one relative to the daemon's data directory.
    pub path: String,
    pub updated_at: i64,
}

/// What a sync target writes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SyncKind {
    #[default]
    Text,
    Tokens,
}

impl SyncKind {
    pub fn key(self) -> &'static str {
        match self {
            SyncKind::Text => "text",
            SyncKind::Tokens => "tokens",
        }
    }

    pub fn parse(value: &str) -> Option<SyncKind> {
        match value {
            "text" => Some(SyncKind::Text),
            "tokens" => Some(SyncKind::Tokens),
            _ => None,
        }
    }
}

/// One element (or the whole token set) that pushes back into a codebase file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncTarget {
    pub id: String,
    pub canvas_id: String,
    pub frame_id: String,
    /// A CSS selector for `text` targets; ignored for `tokens`.
    #[serde(default)]
    pub selector: String,
    pub path: String,
    #[serde(default)]
    pub kind: SyncKind,
    #[serde(default)]
    pub last_synced_at: Option<i64>,
    pub created_at: i64,
}

/// Where a frame stands with its human reviewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Approval {
    #[default]
    Draft,
    ReadyForDev,
    NeedsReview,
}

/// A canvas: pages, tokens, guides, decisions. Frames live beside it so a
/// streamed edit never rewrites the whole document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Canvas {
    pub id: String,
    pub name: String,
    pub pages: Vec<Page>,
    pub tokens: Vec<Token>,
    /// Theme names in display order (`light`, `dark`, …).
    pub token_themes: Vec<String>,
    pub guidelines: Vec<Guideline>,
    pub decisions: Vec<Decision>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Canvas {
    pub fn new(name: impl Into<String>) -> Canvas {
        let now = now_ms();
        Canvas {
            id: new_id(),
            name: name.into(),
            pages: vec![Page::new("Page 1")],
            tokens: Vec::new(),
            token_themes: vec!["light".into(), "dark".into()],
            guidelines: Vec::new(),
            decisions: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn page(&self, id: &str) -> Option<&Page> {
        self.pages.iter().find(|p| p.id == id)
    }

    pub fn theme_names(&self) -> Vec<String> {
        if self.token_themes.is_empty() {
            vec!["default".into()]
        } else {
            self.token_themes.clone()
        }
    }
}

/// One frame: an HTML artboard on a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub id: String,
    pub canvas_id: String,
    pub page_id: String,
    pub name: String,
    pub html: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// The paper this frame sits on; `None` follows the theme.
    #[serde(default)]
    pub background: Option<String>,
    /// `light` / `dark`: sets `data-omapaper-theme` on the document.
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub approval: Approval,
    #[serde(default)]
    pub approval_by: Option<String>,
    pub revision: i64,
    /// Grid position within its page.
    pub ord: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Frame {
    pub fn new(canvas_id: &str, page_id: &str, name: impl Into<String>) -> Frame {
        let now = now_ms();
        Frame {
            id: new_id(),
            canvas_id: canvas_id.into(),
            page_id: page_id.into(),
            name: name.into(),
            html: blank_frame_html(),
            x: 0.0,
            y: 0.0,
            width: 390.0,
            height: 844.0,
            background: None,
            theme: None,
            locked: false,
            approval: Approval::Draft,
            approval_by: None,
            revision: 1,
            ord: 0,
            created_at: now,
            updated_at: now,
        }
    }
}

/// The starting document of every new frame.
pub fn blank_frame_html() -> String {
    "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>Frame</title>\n</head>\n<body>\n  <main style=\"font-family: 'iA Writer Mono S', ui-monospace, monospace; padding: 32px;\">\n    <p style=\"opacity: .6; font-size: 12px; letter-spacing: .08em; text-transform: uppercase;\">OmaPaper</p>\n    <h1 style=\"font-size: 28px; margin: 8px 0;\">An empty frame</h1>\n    <p style=\"opacity: .7; line-height: 1.6;\">Ask an agent to design here, or write HTML in the panel.</p>\n  </main>\n</body>\n</html>\n"
        .to_string()
}

/// A reply inside a comment thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reply {
    pub id: String,
    pub author: String,
    pub body: String,
    pub created_at: i64,
}

/// A thread pinned to an element of a frame, or to a point on a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub canvas_id: String,
    #[serde(default)]
    pub frame_id: Option<String>,
    #[serde(default)]
    pub page_id: Option<String>,
    /// The `data-op-id` of the element it points at.
    #[serde(default)]
    pub element_id: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    pub body: String,
    pub author: String,
    #[serde(default)]
    pub resolved: bool,
    pub created_at: i64,
    #[serde(default)]
    pub replies: Vec<Reply>,
}

/// A saved state of a frame, filed before every replace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: String,
    pub frame_id: String,
    pub html: String,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub label: Option<String>,
    pub created_at: i64,
}

/// The light row of the canvases list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasSummary {
    pub id: String,
    pub name: String,
    pub pages: usize,
    pub frames: usize,
    pub updated_at: i64,
}

/// What somebody has selected in a canvas tab (for `get_selection`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub canvas_id: String,
    #[serde(default)]
    pub page_id: Option<String>,
    #[serde(default)]
    pub frame_id: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub peer: Option<String>,
}

/// A peer colour derived from the name, so agents keep one identity.
pub fn color_for(name: &str) -> String {
    let mut hash: u32 = 2166136261;
    for byte in name.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16777619);
    }
    let hue = (hash % 360) as f32;
    // Calm, theme-friendly saturation and lightness.
    let (r, g, b) = hsl_to_rgb(hue, 0.45, 0.62);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to(r), to(g), to(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_short_and_unique() {
        let a = new_id();
        let b = new_id();
        assert_eq!(a.len(), 12);
        assert_ne!(a, b);
    }

    #[test]
    fn a_canvas_starts_with_one_page() {
        let canvas = Canvas::new("Demo");
        assert_eq!(canvas.pages.len(), 1);
        assert_eq!(canvas.pages[0].name, "Page 1");
        assert!(canvas.page(&canvas.pages[0].id).is_some());
    }

    #[test]
    fn peer_colours_are_stable() {
        assert_eq!(color_for("Claude"), color_for("Claude"));
        assert_ne!(color_for("Claude"), color_for("Codex"));
        assert!(color_for("Claude").starts_with('#'));
    }

    #[test]
    fn roles_parse_from_mentions() {
        assert_eq!(CardRole::parse("@design"), Some(CardRole::Design));
        assert_eq!(CardRole::parse("review"), Some(CardRole::Review));
        assert_eq!(CardRole::parse("nope"), None);
        assert_eq!(CardRole::default().key(), "design");
        assert!(CardRole::Copy.persona().contains("copywriter"));
    }

    #[test]
    fn scheduled_automations_become_due() {
        let mut automation = Automation::new("canvas", "Weekly sweep", "Review the hero");
        automation.trigger = AutomationTrigger::Schedule;
        automation.interval_minutes = Some(30);
        assert!(automation.is_due(now_ms()));
        automation.last_run_at = Some(now_ms());
        assert!(!automation.is_due(now_ms()));
        automation.last_run_at = Some(now_ms() - 31 * 60_000);
        assert!(automation.is_due(now_ms()));
        automation.enabled = false;
        assert!(!automation.is_due(now_ms()));
    }

    #[test]
    fn tokens_read_theme_then_default() {
        let mut values = BTreeMap::new();
        values.insert("default".to_string(), "#000".to_string());
        values.insert("dark".to_string(), "#fff".to_string());
        let token = Token {
            name: "ink".into(),
            kind: TokenKind::Color,
            description: String::new(),
            values,
        };
        assert_eq!(token.value_for("dark"), Some("#fff"));
        assert_eq!(token.value_for("light"), Some("#000"));
    }
}
