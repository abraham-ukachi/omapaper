//! # omapaper-core
//!
//! The engine under every OmaPaper surface: the document model, the SQLite
//! store, Omarchy themes, design tokens, the HTML node model and the guide
//! agents read before they design.
//!
//! No UI, no HTTP, no globals — the daemon, the MCP server and the desktop app
//! all speak to these types.

pub mod agent;
pub mod assets;
pub mod data;
pub mod guide;
pub mod html;
pub mod model;
pub mod plugins;
pub mod render;
pub mod store;
pub mod theme;
pub mod tokens;
pub mod update;

pub use model::{
    blank_frame_html, color_for, new_id, now_ms, Approval, Automation, AutomationTrigger, BoardCard,
    Canvas, CanvasSummary, CardRole, CardStatus, Comment, DataSource, Decision, Frame, Guideline,
    Page, Peer, PeerKind, Reply, Selection, SourceKind, SyncKind, SyncTarget, Token, TokenKind,
    Version,
};
pub use store::{Result as StoreResult, Store, StoreError};
pub use theme::{Palette, Rgb, Theme};

/// The version of the engine (`Cargo.toml`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Stable REST/MCP API version. Bump only on breaking changes.
pub const API_VERSION: &str = "1";

/// What `/api/health` reports.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub name: &'static str,
    pub version: &'static str,
    pub storage: String,
    pub theme: String,
    pub canvases: usize,
}
