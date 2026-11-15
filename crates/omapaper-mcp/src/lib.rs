//! # omapaper-mcp
//!
//! OmaPaper's agent surface: one JSON-RPC server over stdio and HTTP, speaking
//! the Model Context Protocol with the union of Doop's canvas tools and Paper's
//! node tools.

pub mod presence;
pub mod render;
pub mod tools;

use omapaper_core::{Canvas, Frame, Store};
use presence::{Events, NoEvents, NoPresence, Presence};
use render::Renderer;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

/// What every tool call needs: the store, presence, live events, a renderer
/// and a place to write exports.
#[derive(Clone)]
pub struct Host {
    pub store: Store,
    pub presence: Arc<dyn Presence>,
    pub events: Arc<dyn Events>,
    pub renderer: Renderer,
    pub data_dir: PathBuf,
}

impl Host {
    /// A host on the local database (`$OMAPPAPER_DATA_DIR`, else
    /// `~/.local/share/omapaper`).
    pub fn local() -> Host {
        let data_dir = std::env::var_os("OMAPPAPER_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Store::default_path()
                    .parent()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."))
            });
        let path = data_dir.join("omapaper.db");
        let store = Store::open(&path).unwrap_or_else(|error| {
            panic!("cannot open {}: {error}", path.display());
        });
        Host::with_parts(store, data_dir, Arc::new(NoPresence), Arc::new(NoEvents))
    }

    pub fn with_parts(
        store: Store,
        data_dir: impl Into<PathBuf>,
        presence: Arc<dyn Presence>,
        events: Arc<dyn Events>,
    ) -> Host {
        let data_dir = data_dir.into();
        let renderer = Renderer::detect(data_dir.join("render"));
        Host {
            store,
            presence,
            events,
            renderer,
            data_dir,
        }
    }

    /// Finds the canvas a tool means: the given one, the only one, or a fresh
    /// "Untitled canvas" when the database is empty.
    pub fn resolve_canvas(&self, id: Option<&str>) -> Result<Canvas, String> {
        if let Some(id) = id {
            return self
                .store
                .get_canvas(id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("no canvas `{id}`"));
        }
        let list = self.store.list_canvases().map_err(|e| e.to_string())?;
        match list.len() {
            0 => self
                .store
                .create_canvas("Untitled canvas")
                .map_err(|e| e.to_string()),
            1 => self
                .store
                .get_canvas(&list[0].id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "the only canvas vanished".to_string()),
            _ => Err("several canvases exist: pass `canvasId` (list_canvases shows them)".into()),
        }
    }

    pub fn emit(&self, canvas_id: &str, event: Value) {
        self.events.emit(canvas_id, event);
    }

    pub fn touch_canvas(&self, canvas_id: &str) {
        let _ = self.store.touch_canvas(canvas_id);
    }

    /// Saves a frame, bumps its canvas and tells every open tab.
    pub fn save_frame(&self, frame: &Frame) -> Result<(), String> {
        self.store.save_frame(frame).map_err(|e| e.to_string())?;
        self.touch_canvas(&frame.canvas_id);
        self.emit(
            &frame.canvas_id,
            json!({"type": "frame.updated", "frame": frame}),
        );
        Ok(())
    }
}

/// The MCP protocol version this server implements.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Debug, Clone)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    fn method_not_found(method: &str) -> RpcError {
        RpcError {
            code: -32601,
            message: format!("unknown method `{method}`"),
        }
    }

    fn invalid_params(message: impl Into<String>) -> RpcError {
        RpcError {
            code: -32602,
            message: message.into(),
        }
    }
}

/// Handles one JSON-RPC message (or a batch). `None` means "no reply": a
/// notification, or a batch of notifications.
pub fn handle(host: &Host, message: &Value) -> Option<Value> {
    if let Some(batch) = message.as_array() {
        let replies: Vec<Value> = batch.iter().filter_map(|m| handle_one(host, m)).collect();
        return if replies.is_empty() {
            None
        } else {
            Some(Value::Array(replies))
        };
    }
    handle_one(host, message)
}

fn handle_one(host: &Host, message: &Value) -> Option<Value> {
    let object = message.as_object()?;
    let method = object.get("method")?.as_str()?;
    let id = object.get("id").cloned();
    let params = object.get("params").cloned().unwrap_or_else(|| json!({}));

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {
                "name": "omapaper",
                "title": "OmaPaper",
                "version": omapaper_core::VERSION,
            },
            "instructions": omapaper_core::guide::INSTRUCTIONS,
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools::definitions()})),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            if name.is_empty() {
                Err(RpcError::invalid_params("`name` is required"))
            } else if !tools::definitions().iter().any(|tool| tool["name"] == name) {
                Err(RpcError::invalid_params(format!("unknown tool `{name}`")))
            } else {
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match tools::call(host, name, &arguments) {
                    Ok(content) => Ok(json!({"content": content, "isError": false})),
                    Err(error) => Ok(json!({
                        "content": [{"type": "text", "text": error}],
                        "isError": true
                    })),
                }
            }
        }
        other => Err(RpcError::method_not_found(other)),
    };

    let id = id?;
    Some(match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": error.code, "message": error.message}
        }),
    })
}

/// Handles a line of stdio JSON-RPC, returning the line to write back.
pub fn handle_line(host: &Host, line: &str) -> Option<String> {
    let message: Value = serde_json::from_str(line).ok()?;
    handle(host, &message).map(|reply| reply.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Host {
        Host::with_parts(
            Store::open_memory().expect("memory store"),
            std::env::temp_dir().join("omapaper-test"),
            Arc::new(NoPresence),
            Arc::new(NoEvents),
        )
    }

    fn call(host: &Host, name: &str, args: Value) -> Value {
        let request = json!({
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": {"name": name, "arguments": args}
        });
        handle(host, &request).expect("a reply")
    }

    fn payload(reply: &Value) -> Value {
        let text = reply["result"]["content"][0]["text"]
            .as_str()
            .expect("text content");
        serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.into()))
    }

    #[test]
    fn initialize_announces_the_guide() {
        let reply = handle(
            &host(),
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        )
        .unwrap();
        assert_eq!(reply["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(reply["result"]["serverInfo"]["name"], "omapaper");
        assert!(reply["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("get_frame_screenshot"));
    }

    #[test]
    fn notifications_get_no_reply() {
        assert!(handle(
            &host(),
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .is_none());
        assert!(handle(&host(), &json!({"jsonrpc": "2.0", "method": "ping"})).is_none());
    }

    #[test]
    fn tools_are_listed_with_schemas() {
        let reply = handle(
            &host(),
            &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        )
        .unwrap();
        let tools = reply["result"]["tools"].as_array().unwrap();
        assert!(tools.len() >= 40, "{} tools", tools.len());
        for tool in tools {
            assert!(tool["inputSchema"]["type"] == "object", "{tool}");
        }
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for expected in [
            "create_frame",
            "append_frame_html",
            "get_frame_screenshot",
            "find_nodes",
            "get_computed_styles",
            "get_jsx",
            "set_tokens",
        ] {
            assert!(names.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn a_canvas_can_be_created_and_read() {
        let host = host();
        let created = call(&host, "create_canvas", json!({"name": "Travel"}));
        assert_eq!(created["result"]["isError"], false);
        let canvas = payload(&created);
        assert_eq!(canvas["name"], "Travel");
        let map = payload(&call(&host, "get_canvas", json!({})));
        assert_eq!(map["canvas"]["name"], "Travel");
    }

    #[test]
    fn a_frame_streams_and_versions() {
        let host = host();
        call(&host, "create_canvas", json!({"name": "Demo"}));
        let frame = payload(&call(&host, "create_frame", json!({"name": "Hero"})));
        let id = frame["frame"]["id"].as_str().unwrap().to_string();

        call(
            &host,
            "append_frame_html",
            json!({"frameId": id, "html": "<main><h1>Hi</h1>", "start": true}),
        );
        let done = call(
            &host,
            "append_frame_html",
            json!({"frameId": id, "html": "</main>", "done": true}),
        );
        assert_eq!(done["result"]["isError"], false);

        let frame = payload(&call(&host, "get_frame", json!({"frameId": id})));
        assert!(frame["frame"]["html"]
            .as_str()
            .unwrap()
            .contains("<h1>Hi</h1>"));
        assert_eq!(frame["elements"], 2);

        let versions = payload(&call(&host, "list_frame_versions", json!({"frameId": id})));
        assert!(!versions.as_array().unwrap().is_empty());
    }

    #[test]
    fn node_tools_edit_the_document() {
        let host = host();
        call(&host, "create_canvas", json!({"name": "Demo"}));
        let frame = payload(&call(&host, "create_frame", json!({"name": "Card"})));
        let id = frame["frame"]["id"].as_str().unwrap().to_string();
        call(
            &host,
            "set_frame_html",
            json!({"frameId": id, "html": "<main><h1 class=\"t\" data-op-id=\"title\">Old</h1></main>"}),
        );

        let found = payload(&call(
            &host,
            "find_nodes",
            json!({"frameId": id, "selector": ".t"}),
        ));
        assert_eq!(found["count"], 1);
        assert_eq!(found["nodes"][0]["id"], "title");

        call(
            &host,
            "set_text_content",
            json!({"frameId": id, "nodeId": "title", "text": "New"}),
        );
        call(
            &host,
            "update_styles",
            json!({"frameId": id, "selector": ".t", "styles": {"fontSize": "40px"}}),
        );
        let frame = payload(&call(&host, "get_frame", json!({"frameId": id})));
        let html = frame["frame"]["html"].as_str().unwrap();
        assert!(html.contains("New"));
        assert!(html.contains("font-size: 40px"));
    }

    #[test]
    fn tokens_round_trip() {
        let host = host();
        call(&host, "create_canvas", json!({"name": "Demo"}));
        call(
            &host,
            "create_tokens",
            json!({"tokens": [
                {"name": "color-brand", "kind": "color", "values": {"light": "#111", "dark": "#eee"}}
            ]}),
        );
        let css = call(&host, "get_tokens", json!({"format": "css"}));
        let text = css["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("--color-brand: #111"));
    }

    #[test]
    fn unknown_tools_are_errors() {
        let reply = handle(
            &host(),
            &json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                    "params": {"name": "nope", "arguments": {}}}),
        )
        .unwrap();
        assert_eq!(reply["error"]["code"], -32602);
    }
}
