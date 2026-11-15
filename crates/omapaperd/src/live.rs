//! Live state: who is here, what they have selected, and the event bus every
//! open canvas tab and desktop window listens to.

use omapaper_core::{Peer, PeerKind, Selection};
use omapaper_mcp::presence::{Events, Presence};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

const HUMAN_TTL_MS: i64 = 90_000;
const AGENT_TTL_MS: i64 = 30 * 60_000;

pub struct Live {
    peers: Mutex<HashMap<String, HashMap<String, Peer>>>,
    selections: Mutex<HashMap<String, Selection>>,
    tx: broadcast::Sender<Value>,
}

impl Live {
    pub fn new() -> Arc<Live> {
        let (tx, _) = broadcast::channel(512);
        Arc::new(Live {
            peers: Mutex::new(HashMap::new()),
            selections: Mutex::new(HashMap::new()),
            tx,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.tx.subscribe()
    }

    pub fn send(&self, value: Value) {
        let _ = self.tx.send(value);
    }

    pub fn set_selection(&self, canvas_id: &str, selection: Selection) {
        self.selections
            .lock()
            .expect("selections")
            .insert(canvas_id.to_string(), selection.clone());
        self.send(json!({
            "type": "selection",
            "canvasId": canvas_id,
            "selection": selection
        }));
    }

    pub fn remove_peer(&self, canvas_id: &str, peer_id: &str) {
        let removed = self
            .peers
            .lock()
            .expect("peers")
            .get_mut(canvas_id)
            .map(|peers| peers.remove(peer_id).is_some())
            .unwrap_or(false);
        if removed {
            self.broadcast_peers(canvas_id);
        }
    }

    pub fn broadcast_peers(&self, canvas_id: &str) {
        self.send(json!({
            "type": "presence",
            "canvasId": canvas_id,
            "peers": self.list(canvas_id)
        }));
    }

    /// Drops peers that stopped talking, and selections that point nowhere.
    pub fn prune(&self) {
        let now = now_ms();
        let mut canvases = Vec::new();
        {
            let mut peers = self.peers.lock().expect("peers");
            for (canvas_id, group) in peers.iter_mut() {
                let before = group.len();
                group.retain(|_, peer| {
                    let ttl = match peer.kind {
                        PeerKind::Agent => AGENT_TTL_MS,
                        PeerKind::Human => HUMAN_TTL_MS,
                    };
                    now - peer.updated_at < ttl
                });
                if group.len() != before {
                    canvases.push(canvas_id.clone());
                }
            }
        }
        for canvas_id in canvases {
            self.broadcast_peers(&canvas_id);
        }
    }

    pub fn broadcast_theme(&self, theme: &omapaper_core::Theme) {
        self.send(json!({
            "type": "theme.changed",
            "canvasId": "*",
            "theme": theme
        }));
    }
}

impl Presence for Live {
    fn set(&self, canvas_id: &str, peer: Peer) {
        let mut peers = self.peers.lock().expect("peers");
        let group = peers.entry(canvas_id.to_string()).or_default();
        if group.len() > 200 && !group.contains_key(&peer.id) {
            group.clear();
        }
        group.insert(peer.id.clone(), peer);
        drop(peers);
        self.broadcast_peers(canvas_id);
    }

    fn list(&self, canvas_id: &str) -> Vec<Peer> {
        let mut peers: Vec<Peer> = self
            .peers
            .lock()
            .expect("peers")
            .get(canvas_id)
            .map(|group| group.values().cloned().collect())
            .unwrap_or_default();
        peers.sort_by_key(|peer| std::cmp::Reverse(peer.updated_at));
        peers
    }

    fn selection(&self, canvas_id: &str) -> Option<Selection> {
        self.selections
            .lock()
            .expect("selections")
            .get(canvas_id)
            .cloned()
    }
}

impl Events for Live {
    fn emit(&self, canvas_id: &str, mut event: Value) {
        if let Some(map) = event.as_object_mut() {
            map.insert("canvasId".into(), canvas_id.into());
        }
        self.send(event);
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
