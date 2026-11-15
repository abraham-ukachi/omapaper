//! # omapaper-core
//!
//! The engine under every OmaPaper surface: the document model, the SQLite
//! store, Omarchy themes, design tokens, the HTML node model and the guide
//! agents read before they design.
//!
//! No UI, no HTTP, no globals — the daemon, the MCP server and the desktop app
//! all speak to these types.

pub mod guide;
pub mod html;
pub mod model;
pub mod render;
pub mod store;
pub mod theme;
pub mod tokens;

pub use model::{
    blank_frame_html, color_for, new_id, now_ms, Approval, Canvas, CanvasSummary, Comment,
    Decision, Frame, Guideline, Page, Peer, PeerKind, Reply, Selection, Token, TokenKind, Version,
};
pub use store::{Result as StoreResult, Store, StoreError};
pub use theme::{Palette, Rgb, Theme};

/// The version of the engine (`Cargo.toml`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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
