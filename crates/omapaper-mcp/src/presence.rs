//! Presence and live events, behind traits so the MCP layer stays independent
//! of the daemon's WebSockets (and tests can use the no-ops).

use omapaper_core::{Peer, Selection};
use serde_json::Value;

/// Who is on a canvas right now.
pub trait Presence: Send + Sync {
    fn set(&self, canvas_id: &str, peer: Peer);
    fn list(&self, canvas_id: &str) -> Vec<Peer>;
    /// What the human is looking at (frame/node), for `get_selection`.
    fn selection(&self, canvas_id: &str) -> Option<Selection>;
}

/// Nobody: used by the stdio server and in tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoPresence;

impl Presence for NoPresence {
    fn set(&self, _canvas_id: &str, _peer: Peer) {}
    fn list(&self, _canvas_id: &str) -> Vec<Peer> {
        Vec::new()
    }
    fn selection(&self, _canvas_id: &str) -> Option<Selection> {
        None
    }
}

/// Where live updates go.
pub trait Events: Send + Sync {
    fn emit(&self, canvas_id: &str, event: Value);
}

/// Nothing happens: the stdio server has no window watching.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoEvents;

impl Events for NoEvents {
    fn emit(&self, _canvas_id: &str, _event: Value) {}
}
