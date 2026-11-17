//! The local HTTP API: a small REST surface for the canvas UI and desktop app,
//! a WebSocket for presence and live edits, and `/mcp` for agents that prefer
//! HTTP to stdio.

use crate::live::Live;
use crate::workflow;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use omapaper_core::html;
use omapaper_core::model::{
    color_for, new_id, now_ms, Approval, Automation, AutomationTrigger, BoardCard, Canvas,
    CardRole, CardStatus, Comment, DataSource, Frame, Peer, PeerKind, Reply, Selection, SourceKind,
    SyncKind, SyncTarget,
};
use omapaper_core::{render, theme::Theme, tokens, Health, API_VERSION, VERSION};
use omapaper_mcp::Host;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

const INDEX_HTML: &str = include_str!("../../../canvas/index.html");
const APP_CSS: &str = include_str!("../../../canvas/app.css");
const APP_JS: &str = include_str!("../../../canvas/app.js");

pub struct App {
    pub host: Host,
    pub live: Arc<Live>,
    pub data_dir: PathBuf,
    pub plugins_dir: PathBuf,
    pub port: u16,
}

impl App {
    pub fn theme(&self) -> Theme {
        Theme::live_omarchy().unwrap_or_default()
    }

    pub fn tokens_css(&self, canvas: &Canvas) -> String {
        tokens::tokens_css_all(&canvas.tokens, &canvas.theme_names())
    }
}

pub type Shared = Arc<App>;

pub fn router(app: Shared) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.css", get(css))
        .route("/app.js", get(js))
        .route("/health", get(health))
        .route("/api/version", get(version_info))
        .route("/api/plugins", get(list_plugins))
        .route("/api/plugins/{name}/run", post(run_plugin))
        .route("/api/update", get(update_info))
        .route("/api/info", get(info))
        .route("/api/network", get(network_info))
        .route("/api/themes", get(themes))
        .route("/api/theme", get(theme))
        .route("/theme.css", get(theme_css))
        .route("/api/canvases", get(list_canvases).post(create_canvas))
        .route("/api/canvases/{id}/duplicate", post(duplicate_canvas))
        .route(
            "/api/canvases/{id}",
            get(get_canvas).patch(patch_canvas).delete(delete_canvas),
        )
        .route("/api/canvases/{id}/pages", post(create_page))
        .route(
            "/api/canvases/{id}/pages/{page}",
            patch(patch_page).delete(delete_page),
        )
        .route(
            "/api/canvases/{id}/frames",
            get(list_frames).post(create_frame),
        )
        .route(
            "/api/canvases/{id}/comments",
            get(list_comments).post(create_comment),
        )
        .route(
            "/api/canvases/{id}/webhooks",
            get(list_webhooks).post(create_webhook),
        )
        .route("/api/webhooks/{id}", axum::routing::delete(delete_webhook))
        .route("/api/agent", get(agent_status))
        .route(
            "/api/canvases/{id}/cards",
            get(list_cards).post(create_card),
        )
        .route("/api/cards/{id}", patch(patch_card).delete(delete_card))
        .route("/api/cards/{id}/run", post(run_card))
        .route(
            "/api/canvases/{id}/sources",
            get(list_sources).post(create_source),
        )
        .route("/api/sources/{id}", delete(delete_source))
        .route("/api/canvases/{id}/pull", post(pull_data))
        .route(
            "/api/canvases/{id}/syncs",
            get(list_syncs).post(create_sync),
        )
        .route("/api/syncs/{id}", delete(delete_sync))
        .route("/api/syncs/{id}/push", post(push_sync))
        .route(
            "/api/canvases/{id}/automations",
            get(list_automations).post(create_automation),
        )
        .route(
            "/api/automations/{id}",
            patch(patch_automation).delete(delete_automation),
        )
        .route(
            "/api/canvases/{id}/presence",
            get(get_presence).post(post_presence),
        )
        .route(
            "/api/canvases/{id}/selection",
            get(get_selection).post(post_selection),
        )
        .route(
            "/api/frames/{id}",
            get(get_frame).patch(patch_frame).delete(delete_frame),
        )
        .route("/api/frames/{id}/html", get(frame_html))
        .route("/api/frames/{id}/versions", get(frame_versions))
        .route(
            "/api/frames/{id}/versions/{version}/restore",
            post(restore_version),
        )
        .route("/api/frames/{id}/screenshot", get(frame_screenshot))
        .route("/api/frames/{id}/svg", post(insert_frame_svg))
        .route("/api/frames/{id}/export", get(frame_export))
        .route("/i/{filename}", get(live_image))
        .route("/api/comments/{id}", patch(patch_comment))
        .route("/api/comments/{id}/replies", post(reply_comment))
        .route("/mcp", post(mcp))
        .route("/ws", get(ws))
        .with_state(app)
}

// ---- errors ---------------------------------------------------------------

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> ApiError {
        ApiError {
            status,
            message: message.into(),
        }
    }

    pub fn bad(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn not_found(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::NOT_FOUND, message)
    }

    pub fn internal(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({"error": self.message}))).into_response()
    }
}

impl From<omapaper_core::StoreError> for ApiError {
    fn from(error: omapaper_core::StoreError) -> ApiError {
        ApiError::internal(error.to_string())
    }
}

type ApiResult<T> = std::result::Result<T, ApiError>;

// ---- pages and theme ------------------------------------------------------

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn css() -> Response {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], APP_CSS).into_response()
}

async fn js() -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        APP_JS,
    )
        .into_response()
}

async fn health(State(app): State<Shared>) -> Json<Health> {
    let canvases = app.host.store.list_canvases().map(|c| c.len()).unwrap_or(0);
    let storage = app
        .host
        .store
        .path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| app.data_dir.display().to_string());
    Json(Health {
        name: "omapaperd",
        version: VERSION,
        storage,
        theme: app.theme().name,
        canvases,
    })
}

async fn info(State(app): State<Shared>) -> Json<Value> {
    let theme = app.theme();
    Json(json!({
        "name": "OmaPaper",
        "version": VERSION,
        "apiVersion": API_VERSION,
        "theme": theme,
        "themes": Theme::catalogue(Some(theme.clone())).iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
        "renderer": app.host.renderer.name(),
        "dataDir": app.data_dir,
        "protocol": omapaper_mcp::PROTOCOL_VERSION,
    }))
}

async fn version_info(State(app): State<Shared>) -> Json<Value> {
    Json(json!({
        "name": "OmaPaper",
        "version": VERSION,
        "apiVersion": API_VERSION,
        "protocol": omapaper_mcp::PROTOCOL_VERSION,
        "pluginsDir": app.plugins_dir,
    }))
}

async fn list_plugins(State(app): State<Shared>) -> Json<Value> {
    Json(json!({ "plugins": omapaper_core::plugins::describe(&app.plugins_dir) }))
}

#[derive(Deserialize, Default)]
struct RunPlugin {
    input: Option<Value>,
}

/// Runs one plugin with the posted input on its stdin. Explicit, local, and
/// only for manifests that declare a `command`.
async fn run_plugin(
    State(app): State<Shared>,
    Path(name): Path<String>,
    body: Option<Json<RunPlugin>>,
) -> ApiResult<Json<Value>> {
    let input = body
        .and_then(|Json(body)| body.input)
        .unwrap_or_else(|| json!({ "canvasId": null }));
    let dir = app.plugins_dir.clone();
    let run = tokio::task::spawn_blocking(move || omapaper_core::plugins::run(&dir, &name, &input))
        .await
        .map_err(|error| ApiError::bad(error.to_string()))?
        .map_err(ApiError::bad)?;
    Ok(Json(json!(run)))
}

/// The latest GitHub release, compared with this build.
async fn update_info() -> ApiResult<Json<Value>> {
    let info = tokio::task::spawn_blocking(|| {
        omapaper_core::update::check("abraham-ukachi/omapaper", VERSION)
    })
    .await
    .map_err(|error| ApiError::bad(error.to_string()))?
    .map_err(ApiError::bad)?;
    Ok(Json(json!(info)))
}

async fn network_info(State(app): State<Shared>) -> Json<Value> {
    let mut addresses = Vec::new();
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(address) = socket.local_addr() {
                let ip = address.ip();
                if !ip.is_loopback() {
                    addresses.push(ip.to_string());
                }
            }
        }
    }
    Json(json!({
        "port": app.port,
        "addresses": addresses,
    }))
}

async fn themes(State(app): State<Shared>) -> Json<Value> {
    let live = app.theme();
    Json(json!({
        "live": live.name,
        "themes": Theme::catalogue(Some(live)),
    }))
}

#[derive(Deserialize)]
struct ThemeQuery {
    name: Option<String>,
}

async fn theme(State(app): State<Shared>, Query(query): Query<ThemeQuery>) -> Json<Value> {
    let live = app.theme();
    let theme = match query.name.as_deref() {
        None | Some("live") | Some("omarchy") => live.clone(),
        Some(name) => Theme::catalogue(Some(live.clone()))
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .unwrap_or(live),
    };
    Json(json!({
        "theme": theme,
        "css": theme.css_vars(),
    }))
}

async fn theme_css(State(app): State<Shared>) -> Response {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        app.theme().css_vars(),
    )
        .into_response()
}

// ---- canvases -------------------------------------------------------------

async fn list_canvases(State(app): State<Shared>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(app.host.store.list_canvases()?)))
}

#[derive(Deserialize)]
struct CreateCanvas {
    name: String,
}

async fn create_canvas(
    State(app): State<Shared>,
    Json(body): Json<CreateCanvas>,
) -> ApiResult<Json<Canvas>> {
    let canvas = app.host.store.create_canvas(&body.name)?;
    app.live
        .send(json!({"type": "canvas.created", "canvas": canvas}));
    Ok(Json(canvas))
}

async fn get_canvas(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let canvas = app
        .host
        .store
        .get_canvas(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no canvas `{id}`")))?;
    let frames: Vec<Value> = app
        .host
        .store
        .frames(&id)?
        .iter()
        .map(frame_summary)
        .collect();
    let comments = app.host.store.comments(&id)?;
    Ok(Json(json!({
        "canvas": canvas,
        "frames": frames,
        "comments": comments,
    })))
}

#[derive(Deserialize)]
struct PatchCanvas {
    name: Option<String>,
    guidelines: Option<Vec<omapaper_core::model::Guideline>>,
    decisions: Option<Vec<omapaper_core::model::Decision>>,
}

async fn patch_canvas(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<PatchCanvas>,
) -> ApiResult<Json<Canvas>> {
    let mut canvas = app
        .host
        .store
        .get_canvas(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no canvas `{id}`")))?;
    if let Some(name) = body.name {
        canvas.name = name;
    }
    if let Some(guidelines) = body.guidelines {
        canvas.guidelines = guidelines;
    }
    if let Some(decisions) = body.decisions {
        canvas.decisions = decisions;
    }
    app.host.store.save_canvas(&canvas)?;
    app.live
        .send(json!({"type": "canvas.updated", "canvas": canvas}));
    Ok(Json(canvas))
}

async fn delete_canvas(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    app.host.store.delete_canvas(&id)?;
    app.live
        .send(json!({"type": "canvas.deleted", "canvasId": id}));
    Ok(StatusCode::NO_CONTENT)
}

async fn duplicate_canvas(
    State(app): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<Canvas>> {
    let source = app
        .host
        .store
        .get_canvas(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no canvas `{id}`")))?;
    let mut canvas = app
        .host
        .store
        .create_canvas(&format!("{} copy", source.name))?;
    canvas.pages = source.pages;
    canvas.tokens = source.tokens;
    canvas.token_themes = source.token_themes;
    canvas.guidelines = source.guidelines;
    canvas.decisions = source.decisions;
    app.host.store.save_canvas(&canvas)?;

    for mut frame in app.host.store.frames(&id)? {
        frame.id = new_id();
        frame.canvas_id = canvas.id.clone();
        app.host.store.insert_frame(&frame)?;
    }
    for mut comment in app.host.store.comments(&id)? {
        comment.id = new_id();
        comment.canvas_id = canvas.id.clone();
        app.host.store.add_comment(&comment)?;
    }
    app.live
        .send(json!({"type": "canvas.created", "canvas": canvas}));
    Ok(Json(canvas))
}

#[derive(Deserialize)]
struct CreatePage {
    name: Option<String>,
}

#[derive(Deserialize)]
struct PatchPage {
    name: Option<String>,
}

async fn create_page(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<CreatePage>,
) -> ApiResult<Json<Canvas>> {
    let mut canvas = app
        .host
        .store
        .get_canvas(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no canvas `{id}`")))?;
    let name = body
        .name
        .unwrap_or_else(|| format!("Page {}", canvas.pages.len() + 1));
    canvas.pages.push(omapaper_core::model::Page::new(name));
    app.host.store.save_canvas(&canvas)?;
    app.live
        .send(json!({"type": "canvas.updated", "canvas": canvas}));
    Ok(Json(canvas))
}

async fn patch_page(
    State(app): State<Shared>,
    Path((id, page_id)): Path<(String, String)>,
    Json(body): Json<PatchPage>,
) -> ApiResult<Json<Canvas>> {
    let mut canvas = app
        .host
        .store
        .get_canvas(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no canvas `{id}`")))?;
    let page = canvas
        .pages
        .iter_mut()
        .find(|page| page.id == page_id)
        .ok_or_else(|| ApiError::not_found(format!("no page `{page_id}`")))?;
    if let Some(name) = body.name {
        page.name = name;
    }
    app.host.store.save_canvas(&canvas)?;
    app.live
        .send(json!({"type": "canvas.updated", "canvas": canvas}));
    Ok(Json(canvas))
}

async fn delete_page(
    State(app): State<Shared>,
    Path((id, page_id)): Path<(String, String)>,
) -> ApiResult<Json<Canvas>> {
    let mut canvas = app
        .host
        .store
        .get_canvas(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no canvas `{id}`")))?;
    if canvas.pages.len() <= 1 {
        return Err(ApiError::bad("cannot delete the last page"));
    }
    let target = canvas
        .pages
        .iter()
        .find(|page| page.id != page_id)
        .map(|page| page.id.clone())
        .ok_or_else(|| ApiError::bad("no page to move frames to"))?;
    // move frames from deleted page to the first remaining page
    let frames = app.host.store.frames(&id)?;
    for mut frame in frames {
        if frame.page_id == page_id {
            frame.page_id = target.clone();
            app.host.store.save_frame(&frame)?;
        }
    }
    canvas.pages.retain(|page| page.id != page_id);
    app.host.store.save_canvas(&canvas)?;
    app.live
        .send(json!({"type": "canvas.updated", "canvas": canvas}));
    // reload frames in UI happens via canvas.updated? canvas has no frames; emit frame list? UI calls loadCanvas on canvas.updated? Check.
    Ok(Json(canvas))
}

// ---- frames ---------------------------------------------------------------

fn frame_summary(frame: &Frame) -> Value {
    json!({
        "id": frame.id,
        "canvasId": frame.canvas_id,
        "pageId": frame.page_id,
        "name": frame.name,
        "x": frame.x,
        "y": frame.y,
        "width": frame.width,
        "height": frame.height,
        "background": frame.background,
        "theme": frame.theme,
        "locked": frame.locked,
        "approval": frame.approval,
        "revision": frame.revision,
        "elements": html::element_count(&frame.html),
        "updatedAt": frame.updated_at,
    })
}

async fn list_frames(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let frames: Vec<Value> = app
        .host
        .store
        .frames(&id)?
        .iter()
        .map(frame_summary)
        .collect();
    Ok(Json(json!(frames)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFrame {
    page_id: Option<String>,
    name: Option<String>,
    width: Option<f64>,
    height: Option<f64>,
    html: Option<String>,
}

async fn create_frame(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<CreateFrame>,
) -> ApiResult<Json<Frame>> {
    let canvas = app
        .host
        .store
        .get_canvas(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no canvas `{id}`")))?;
    let page_id = match body.page_id {
        Some(page_id) if canvas.page(&page_id).is_some() => page_id,
        Some(page_id) => return Err(ApiError::bad(format!("no page `{page_id}`"))),
        None => canvas.pages[0].id.clone(),
    };
    let name = body
        .name
        .unwrap_or_else(|| format!("Frame {}", app.host.store.frame_count(&id).unwrap_or(0) + 1));
    let mut frame = Frame::new(&id, &page_id, name);
    if let Some(width) = body.width {
        frame.width = width;
    }
    if let Some(height) = body.height {
        frame.height = height;
    }
    if let Some(html) = body.html {
        frame.html = html;
    }
    app.host.store.insert_frame(&frame)?;
    app.live
        .send(json!({"type": "frame.created", "frame": frame}));
    Ok(Json(frame))
}

async fn get_frame(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let frame = app
        .host
        .store
        .get_frame(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no frame `{id}`")))?;
    Ok(Json(json!({
        "frame": frame,
        "versions": app.host.store.versions(&id)?.into_iter().map(|v| json!({
            "id": v.id,
            "label": v.label,
            "createdAt": v.created_at,
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PatchFrame {
    name: Option<String>,
    html: Option<String>,
    width: Option<f64>,
    height: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
    #[serde(default, with = "double_option")]
    background: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    theme: Option<Option<String>>,
    locked: Option<bool>,
    approval: Option<String>,
}

mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[allow(dead_code)]
    pub fn serialize<S: Serializer>(
        value: &Option<Option<String>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        value.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<Option<String>>, D::Error> {
        Option::<String>::deserialize(d).map(Some)
    }
}

fn approval(value: &str) -> ApiResult<Approval> {
    match value {
        "draft" => Ok(Approval::Draft),
        "needs_review" => Ok(Approval::NeedsReview),
        "ready_for_dev" => Ok(Approval::ReadyForDev),
        other => Err(ApiError::bad(format!("unknown approval `{other}`"))),
    }
}

async fn patch_frame(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<PatchFrame>,
) -> ApiResult<Json<Frame>> {
    let mut frame = app
        .host
        .store
        .get_frame(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no frame `{id}`")))?;
    if frame.locked && body.html.is_some() {
        return Err(ApiError::bad("frame is locked"));
    }
    if let Some(name) = body.name {
        frame.name = name;
    }
    let was_approved = frame.approval == Approval::ReadyForDev;
    if let Some(document) = body.html {
        if document != frame.html {
            app.host
                .store
                .push_version(&frame, Some("before edit in canvas"), 30)?;
            frame.html = document;
            frame.revision += 1;
            frame.approval = if was_approved {
                Approval::NeedsReview
            } else {
                Approval::Draft
            };
            frame.approval_by = None;
        }
    }
    if let Some(width) = body.width {
        frame.width = width;
        frame.approval = if was_approved {
            Approval::NeedsReview
        } else {
            Approval::Draft
        };
    }
    if let Some(height) = body.height {
        frame.height = height;
        frame.approval = if was_approved {
            Approval::NeedsReview
        } else {
            Approval::Draft
        };
    }
    if let Some(x) = body.x {
        frame.x = x;
    }
    if let Some(y) = body.y {
        frame.y = y;
    }
    if let Some(background) = body.background {
        frame.background = background;
    }
    if let Some(theme) = body.theme {
        frame.theme = theme;
    }
    if let Some(locked) = body.locked {
        frame.locked = locked;
    }
    let mut approved_now = false;
    if let Some(value) = body.approval {
        frame.approval = approval(&value)?;
        frame.approval_by = Some("You".into());
        approved_now = frame.approval == Approval::ReadyForDev;
    }
    app.host.store.save_frame(&frame)?;
    if approved_now {
        workflow::fire_automations(
            &app,
            &frame.canvas_id,
            AutomationTrigger::FrameApproved,
            &format!("`{}` is ready for dev.", frame.name),
        );
    }
    app.host.touch_canvas(&frame.canvas_id);
    app.live
        .send(json!({"type": "frame.updated", "frame": frame}));
    Ok(Json(frame))
}

async fn delete_frame(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let frame = app
        .host
        .store
        .get_frame(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no frame `{id}`")))?;
    app.host.store.delete_frame(&id)?;
    app.live
        .send(json!({"type": "frame.deleted", "frameId": id, "canvasId": frame.canvas_id}));
    Ok(StatusCode::NO_CONTENT)
}

async fn frame_html(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Query(query): Query<ThemeQuery>,
) -> ApiResult<Response> {
    let frame = app
        .host
        .store
        .get_frame(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no frame `{id}`")))?;
    let canvas = app.host.store.get_canvas(&frame.canvas_id)?;
    let tokens_css = canvas
        .as_ref()
        .map(|c| app.tokens_css(c))
        .unwrap_or_default();
    let live = app.theme();
    let theme = match query.name.as_deref() {
        Some(name) if name != "live" => Theme::catalogue(Some(live.clone()))
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .unwrap_or(live),
        _ => live,
    };
    let doc = render::frame_document(&frame, &theme, &tokens_css);
    Ok((
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        doc,
    )
        .into_response())
}

async fn frame_versions(
    State(app): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(app.host.store.versions(&id)?)))
}

async fn restore_version(
    State(app): State<Shared>,
    Path((id, version_id)): Path<(String, String)>,
) -> ApiResult<Json<Frame>> {
    let mut frame = app
        .host
        .store
        .get_frame(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no frame `{id}`")))?;
    let version = app
        .host
        .store
        .version(&version_id)?
        .filter(|v| v.frame_id == id)
        .ok_or_else(|| ApiError::not_found(format!("no version `{version_id}`")))?;
    app.host
        .store
        .push_version(&frame, Some("before restore"), 30)?;
    frame.html = version.html;
    frame.width = version.width;
    frame.height = version.height;
    frame.revision += 1;
    frame.approval = Approval::Draft;
    app.host.store.save_frame(&frame)?;
    app.host.touch_canvas(&frame.canvas_id);
    app.live
        .send(json!({"type": "frame.updated", "frame": frame}));
    Ok(Json(frame))
}

#[derive(Deserialize)]
struct ScaleQuery {
    scale: Option<f64>,
}

async fn frame_screenshot(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Query(query): Query<ScaleQuery>,
) -> ApiResult<Response> {
    let frame = app
        .host
        .store
        .get_frame(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no frame `{id}`")))?;
    let canvas = app.host.store.get_canvas(&frame.canvas_id)?;
    let tokens_css = canvas
        .as_ref()
        .map(|c| app.tokens_css(c))
        .unwrap_or_default();
    let doc = render::frame_document(&frame, &app.theme(), &tokens_css);
    let png = app
        .host
        .renderer
        .screenshot(&doc, frame.width, frame.height, query.scale.unwrap_or(1.0))
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(([(header::CONTENT_TYPE, "image/png")], png).into_response())
}

#[derive(Deserialize)]
struct ExportQuery {
    format: Option<String>,
}

async fn live_image(
    State(app): State<Shared>,
    Path(filename): Path<String>,
) -> ApiResult<Response> {
    let id = filename
        .trim_end_matches(".png")
        .trim_end_matches(".jpg")
        .trim_end_matches(".jpeg")
        .to_string();
    let frame = app
        .host
        .store
        .get_frame(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no frame `{id}`")))?;
    let canvas = app.host.store.get_canvas(&frame.canvas_id)?;
    let tokens_css = canvas
        .as_ref()
        .map(|c| app.tokens_css(c))
        .unwrap_or_default();
    let doc = render::frame_document(&frame, &app.theme(), &tokens_css);
    let png = app
        .host
        .renderer
        .screenshot(&doc, frame.width, frame.height, 2.0)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/png".to_string()),
            (header::CACHE_CONTROL, "no-cache".to_string()),
        ],
        png,
    )
        .into_response())
}

async fn frame_export(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Query(query): Query<ExportQuery>,
) -> ApiResult<Response> {
    let frame = app
        .host
        .store
        .get_frame(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no frame `{id}`")))?;
    let canvas = app.host.store.get_canvas(&frame.canvas_id)?;
    let tokens_css = canvas
        .as_ref()
        .map(|c| app.tokens_css(c))
        .unwrap_or_default();
    let doc = render::frame_document(&frame, &app.theme(), &tokens_css);
    let format = query.format.unwrap_or_else(|| "html".into());
    let (bytes, mime): (Vec<u8>, &str) = match format.as_str() {
        "html" => (doc.into_bytes(), "text/html; charset=utf-8"),
        "png" => (
            app.host
                .renderer
                .screenshot(&doc, frame.width, frame.height, 1.0)
                .map_err(|e| ApiError::internal(e.to_string()))?,
            "image/png",
        ),
        "pdf" => (
            app.host
                .renderer
                .pdf(&doc, frame.width, frame.height)
                .map_err(|e| ApiError::internal(e.to_string()))?,
            "application/pdf",
        ),
        other => return Err(ApiError::bad(format!("unsupported format `{other}`"))),
    };
    Ok((
        [
            (header::CONTENT_TYPE, mime.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}.{format}\"", frame.name),
            ),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewSvg {
    svg: String,
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
    name: Option<String>,
}

async fn insert_frame_svg(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<NewSvg>,
) -> ApiResult<Json<Frame>> {
    let frame = workflow::insert_svg(
        &app,
        &id,
        &workflow::SvgLayer {
            svg: &body.svg,
            x: body.x.unwrap_or(0.0),
            y: body.y.unwrap_or(0.0),
            width: body.width.unwrap_or(0.0),
            height: body.height.unwrap_or(0.0),
            name: body.name.as_deref(),
        },
    )
    .map_err(ApiError::bad)?;
    Ok(Json(frame))
}

// ---- data in and copy out -------------------------------------------------

async fn list_sources(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(app.host.store.data_sources(&id)?)))
}

#[derive(Deserialize)]
struct NewSource {
    name: String,
    path: String,
    kind: Option<String>,
}

async fn create_source(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<NewSource>,
) -> ApiResult<Json<DataSource>> {
    let kind = match body.kind.as_deref() {
        Some(value) => SourceKind::parse(value)
            .ok_or_else(|| ApiError::bad(format!("unknown source kind `{value}`")))?,
        None => infer_kind(&body.path),
    };
    let source = DataSource {
        id: new_id(),
        canvas_id: id,
        name: body.name,
        kind,
        path: body.path,
        updated_at: now_ms(),
    };
    app.host.store.add_data_source(&source)?;
    Ok(Json(source))
}

fn infer_kind(path: &str) -> SourceKind {
    let lower = path.to_lowercase();
    if lower.ends_with(".csv") {
        SourceKind::Csv
    } else if lower.ends_with(".env") || lower.ends_with(".env.local") {
        SourceKind::Env
    } else {
        SourceKind::Json
    }
}

async fn delete_source(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    app.host.store.delete_data_source(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullBody {
    source_id: Option<String>,
}

/// Reads every (or one) source and writes its values into the canvas frames.
async fn pull_data(
    State(app): State<Shared>,
    Path(id): Path<String>,
    body: Option<Json<PullBody>>,
) -> ApiResult<Json<Value>> {
    let requested = body.and_then(|Json(body)| body.source_id);
    let sources = app.host.store.data_sources(&id)?;
    if sources.is_empty() {
        return Err(ApiError::bad("this canvas has no data sources yet"));
    }
    let mut values = BTreeMap::new();
    let mut used = Vec::new();
    for source in sources {
        if requested.as_deref().is_some_and(|only| only != source.id) {
            continue;
        }
        let parsed = omapaper_core::data::read(&source.path, &app.host.data_dir, source.kind)
            .map_err(ApiError::bad)?;
        values.extend(parsed);
        app.host.store.touch_data_source(&source.id)?;
        used.push(source.name);
    }
    let mut frames_changed = 0;
    let mut replaced = 0;
    for mut frame in app.host.store.frames(&id)? {
        let (html, count) = omapaper_core::data::apply(&frame.html, &values);
        if count == 0 || html == frame.html {
            continue;
        }
        let _ = app
            .host
            .store
            .push_version(&frame, Some("before a data pull"), 30);
        frame.html = html;
        frame.revision += 1;
        app.host.store.save_frame(&frame)?;
        app.live.send(json!({
            "type": "frame.updated",
            "canvasId": frame.canvas_id,
            "frame": frame,
        }));
        frames_changed += 1;
        replaced += count;
    }
    Ok(Json(json!({
        "sources": used,
        "keys": values.len(),
        "frames": frames_changed,
        "replaced": replaced,
    })))
}

async fn list_syncs(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(app.host.store.sync_targets(&id)?)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewSync {
    frame_id: String,
    path: String,
    selector: Option<String>,
    kind: Option<String>,
}

async fn create_sync(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<NewSync>,
) -> ApiResult<Json<SyncTarget>> {
    if app.host.store.get_frame(&body.frame_id)?.is_none() {
        return Err(ApiError::not_found(format!("no frame `{}`", body.frame_id)));
    }
    let kind = match body.kind.as_deref() {
        Some(value) => SyncKind::parse(value)
            .ok_or_else(|| ApiError::bad(format!("unknown sync kind `{value}`")))?,
        None => SyncKind::Text,
    };
    let selector = body.selector.unwrap_or_default();
    if kind == SyncKind::Text && selector.trim().is_empty() {
        return Err(ApiError::bad("a text sync needs a CSS selector"));
    }
    let target = SyncTarget {
        id: new_id(),
        canvas_id: id,
        frame_id: body.frame_id,
        selector,
        path: body.path,
        kind,
        last_synced_at: None,
        created_at: now_ms(),
    };
    app.host.store.add_sync_target(&target)?;
    Ok(Json(target))
}

async fn delete_sync(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    app.host.store.delete_sync_target(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Writes the element's text (or the token CSS) to the target's file.
async fn push_sync(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let target = app
        .host
        .store
        .get_sync_target(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no sync target `{id}`")))?;
    let frame = app
        .host
        .store
        .get_frame(&target.frame_id)?
        .ok_or_else(|| ApiError::not_found(format!("no frame `{}`", target.frame_id)))?;
    let content = match target.kind {
        SyncKind::Text => omapaper_core::html::text_of(&frame.html, &target.selector)
            .map_err(|error| ApiError::bad(format!("{error}")))?,
        SyncKind::Tokens => {
            let canvas = app
                .host
                .store
                .get_canvas(&target.canvas_id)?
                .ok_or_else(|| ApiError::not_found(format!("no canvas `{}`", target.canvas_id)))?;
            omapaper_core::tokens::tokens_css_all(&canvas.tokens, &canvas.token_themes)
        }
    };
    let path = omapaper_core::data::resolve(&target.path, &app.host.data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| ApiError::bad(error.to_string()))?;
    }
    std::fs::write(&path, &content)
        .map_err(|error| ApiError::bad(format!("cannot write {}: {error}", path.display())))?;
    app.host.store.touch_sync_target(&id)?;
    Ok(Json(json!({
        "path": path.display().to_string(),
        "bytes": content.len(),
    })))
}

// ---- webhooks -------------------------------------------------------------

async fn list_webhooks(
    State(app): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(app.host.store.webhooks(&id)?)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewWebhook {
    url: String,
    events: Option<Vec<String>>,
}

async fn create_webhook(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<NewWebhook>,
) -> ApiResult<Json<Value>> {
    let webhook = app.host.store.add_webhook(
        &id,
        &body.url,
        body.events
            .as_deref()
            .unwrap_or(&["comment.created".into()]),
    )?;
    Ok(Json(json!(webhook)))
}

async fn delete_webhook(
    State(app): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    app.host.store.delete_webhook(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- agent board ----------------------------------------------------------

async fn agent_status() -> Json<Value> {
    match omapaper_core::agent::AgentConfig::from_env() {
        Some(config) => Json(json!({
            "configured": true,
            "model": config.model,
            "baseUrl": config.base_url,
            "keyVar": "OMAPAPER_AGENT_API_KEY",
        })),
        None => Json(json!({
            "configured": false,
            "model": null,
            "baseUrl": null,
            "keyVar": "OMAPAPER_AGENT_API_KEY",
        })),
    }
}

async fn list_cards(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(app.host.store.cards(&id)?)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewCard {
    title: Option<String>,
    brief: String,
    role: Option<String>,
    frame_id: Option<String>,
    author: Option<String>,
}

async fn create_card(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<NewCard>,
) -> ApiResult<Json<BoardCard>> {
    let role = match body.role.as_deref() {
        Some(value) => CardRole::parse(value)
            .ok_or_else(|| ApiError::bad(format!("unknown role `{value}`")))?,
        None => CardRole::default(),
    };
    let title = body
        .title
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| workflow::title_from(&body.brief));
    let card = workflow::queue_card(
        &app,
        &id,
        &title,
        &body.brief,
        role,
        body.frame_id,
        body.author.as_deref().unwrap_or("You"),
    )?;
    Ok(Json(card))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PatchCard {
    title: Option<String>,
    brief: Option<String>,
    status: Option<String>,
    role: Option<String>,
    result: Option<String>,
}

async fn patch_card(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<PatchCard>,
) -> ApiResult<Json<BoardCard>> {
    let mut card = app
        .host
        .store
        .get_card(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no card `{id}`")))?;
    if let Some(title) = body.title {
        card.title = title;
    }
    if let Some(brief) = body.brief {
        card.brief = brief;
    }
    if let Some(status) = body.status.as_deref() {
        card.status = CardStatus::parse(status)
            .ok_or_else(|| ApiError::bad(format!("unknown status `{status}`")))?;
        if card.status != CardStatus::Failed {
            card.error = None;
        }
    }
    if let Some(role) = body.role.as_deref() {
        card.role =
            CardRole::parse(role).ok_or_else(|| ApiError::bad(format!("unknown role `{role}`")))?;
    }
    if let Some(result) = body.result {
        card.result = Some(result);
    }
    app.host.store.save_card(&card)?;
    app.live.send(json!({
        "type": "card.updated",
        "canvasId": card.canvas_id,
        "card": card,
    }));
    Ok(Json(card))
}

async fn delete_card(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let card = app
        .host
        .store
        .get_card(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no card `{id}`")))?;
    app.host.store.delete_card(&id)?;
    app.live.send(json!({
        "type": "card.deleted",
        "canvasId": card.canvas_id,
        "cardId": id,
    }));
    Ok(StatusCode::NO_CONTENT)
}

async fn run_card(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<BoardCard>> {
    let card = app
        .host
        .store
        .get_card(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no card `{id}`")))?;
    if omapaper_core::agent::AgentConfig::from_env().is_none() {
        return Err(ApiError::bad(
            "no agent key: set OMAPAPER_AGENT_API_KEY (or OPENAI_API_KEY) and restart omapaperd",
        ));
    }
    tokio::spawn(workflow::run_card(app.clone(), id));
    Ok(Json(card))
}

// ---- automations ----------------------------------------------------------

async fn list_automations(
    State(app): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(app.host.store.automations(&id)?)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewAutomation {
    name: String,
    brief: String,
    trigger: Option<String>,
    role: Option<String>,
    interval_minutes: Option<i64>,
}

async fn create_automation(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<NewAutomation>,
) -> ApiResult<Json<Automation>> {
    let mut automation = Automation::new(&id, body.name, body.brief);
    if let Some(trigger) = body.trigger.as_deref() {
        automation.trigger = AutomationTrigger::parse(trigger)
            .ok_or_else(|| ApiError::bad(format!("unknown trigger `{trigger}`")))?;
    }
    if let Some(role) = body.role.as_deref() {
        automation.role =
            CardRole::parse(role).ok_or_else(|| ApiError::bad(format!("unknown role `{role}`")))?;
    }
    automation.interval_minutes = body.interval_minutes.filter(|minutes| *minutes > 0);
    if automation.trigger == AutomationTrigger::Schedule && automation.interval_minutes.is_none() {
        return Err(ApiError::bad(
            "a scheduled automation needs `intervalMinutes`",
        ));
    }
    app.host.store.add_automation(&automation)?;
    Ok(Json(automation))
}

#[derive(Deserialize)]
struct PatchAutomation {
    enabled: Option<bool>,
    brief: Option<String>,
}

async fn patch_automation(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<PatchAutomation>,
) -> ApiResult<Json<Value>> {
    let mut found: Option<Automation> = None;
    for canvas in app.host.store.list_canvases()? {
        if let Some(automation) = app
            .host
            .store
            .automations(&canvas.id)?
            .into_iter()
            .find(|automation| automation.id == id)
        {
            found = Some(automation);
            break;
        }
    }
    let mut automation =
        found.ok_or_else(|| ApiError::not_found(format!("no automation `{id}`")))?;
    if let Some(enabled) = body.enabled {
        automation.enabled = enabled;
    }
    if let Some(brief) = body.brief {
        automation.brief = brief;
    }
    app.host.store.save_automation(&automation)?;
    Ok(Json(json!(automation)))
}

async fn delete_automation(
    State(app): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    app.host.store.delete_automation(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- comments -------------------------------------------------------------

async fn list_comments(
    State(app): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(app.host.store.comments(&id)?)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewComment {
    frame_id: Option<String>,
    element_id: Option<String>,
    page_id: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    body: String,
    author: Option<String>,
}

pub(crate) fn dispatch_webhooks(
    app: Shared,
    canvas_id: String,
    event: &'static str,
    payload: Value,
) {
    let webhooks = app.host.store.webhooks(&canvas_id).unwrap_or_default();
    for webhook in webhooks {
        if !webhook.events.is_empty() && !webhook.events.iter().any(|e| e == event) {
            continue;
        }
        let url = webhook.url.clone();
        let body = json!({ "event": event, "canvasId": canvas_id, "payload": payload }).to_string();
        tokio::task::spawn_blocking(move || {
            let _ = std::process::Command::new("curl")
                .args([
                    "-sS",
                    "-X",
                    "POST",
                    "-H",
                    "content-type: application/json",
                    "-d",
                    &body,
                    &url,
                ])
                .output();
        });
    }
}

async fn create_comment(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<NewComment>,
) -> ApiResult<Json<Comment>> {
    let comment = Comment {
        id: new_id(),
        canvas_id: id,
        frame_id: body.frame_id,
        page_id: body.page_id,
        element_id: body.element_id,
        x: body.x,
        y: body.y,
        body: body.body,
        author: body.author.unwrap_or_else(|| "You".into()),
        resolved: false,
        created_at: now_ms(),
        replies: Vec::new(),
    };
    app.host.store.add_comment(&comment)?;
    if let Some(role) = workflow::mention_role(&comment.body) {
        let _ = workflow::queue_card(
            &app,
            &comment.canvas_id,
            &workflow::title_from(&comment.body),
            &comment.body,
            role,
            comment.frame_id.clone(),
            &format!("@{}", role.key()),
        );
    }
    workflow::fire_automations(
        &app,
        &comment.canvas_id,
        AutomationTrigger::CommentCreated,
        &format!("New comment from {}: {}", comment.author, comment.body),
    );
    let canvas_id = comment.canvas_id.clone();
    dispatch_webhooks(
        app.clone(),
        canvas_id,
        "comment.created",
        json!(comment.clone()),
    );
    app.live
        .send(json!({"type": "comment.created", "comment": comment}));
    Ok(Json(comment))
}

#[derive(Deserialize)]
struct PatchComment {
    resolved: Option<bool>,
}

async fn patch_comment(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<PatchComment>,
) -> ApiResult<Json<Comment>> {
    let mut comment = app
        .host
        .store
        .get_comment(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no comment `{id}`")))?;
    if let Some(resolved) = body.resolved {
        comment.resolved = resolved;
    }
    app.host.store.save_comment(&comment)?;
    app.live
        .send(json!({"type": "comment.updated", "comment": comment}));
    Ok(Json(comment))
}

#[derive(Deserialize)]
struct NewReply {
    body: String,
    author: Option<String>,
}

async fn reply_comment(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<NewReply>,
) -> ApiResult<Json<Comment>> {
    let mut comment = app
        .host
        .store
        .get_comment(&id)?
        .ok_or_else(|| ApiError::not_found(format!("no comment `{id}`")))?;
    comment.replies.push(Reply {
        id: new_id(),
        author: body.author.unwrap_or_else(|| "You".into()),
        body: body.body,
        created_at: now_ms(),
    });
    app.host.store.save_comment(&comment)?;
    app.live
        .send(json!({"type": "comment.updated", "comment": comment}));
    Ok(Json(comment))
}

// ---- presence -------------------------------------------------------------

async fn get_presence(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    use omapaper_mcp::presence::Presence;
    Ok(Json(json!(app.live.list(&id))))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresencePost {
    peer_id: String,
    name: String,
    kind: Option<String>,
    frame_id: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    status: Option<String>,
    selection: Option<Vec<String>>,
}

async fn post_presence(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<PresencePost>,
) -> ApiResult<StatusCode> {
    use omapaper_mcp::presence::Presence;
    app.live.set(
        &id,
        Peer {
            id: body.peer_id,
            name: body.name.clone(),
            kind: if body.kind.as_deref() == Some("agent") {
                PeerKind::Agent
            } else {
                PeerKind::Human
            },
            color: color_for(&body.name),
            frame_id: body.frame_id,
            x: body.x.unwrap_or(0.0),
            y: body.y.unwrap_or(0.0),
            status: body.status,
            selection: body.selection.unwrap_or_default(),
            updated_at: now_ms(),
        },
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn get_selection(
    State(app): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<Selection>> {
    use omapaper_mcp::presence::Presence;
    Ok(Json(app.live.selection(&id).unwrap_or(Selection {
        canvas_id: id,
        ..Selection::default()
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SelectionPost {
    peer_id: String,
    frame_id: Option<String>,
    page_id: Option<String>,
    node_id: Option<String>,
}

async fn post_selection(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<SelectionPost>,
) -> ApiResult<StatusCode> {
    app.live.set_selection(
        &id,
        Selection {
            canvas_id: id.clone(),
            page_id: body.page_id,
            frame_id: body.frame_id,
            node_id: body.node_id,
            peer: Some(body.peer_id),
        },
    );
    Ok(StatusCode::NO_CONTENT)
}

// ---- MCP over HTTP --------------------------------------------------------

async fn mcp(State(app): State<Shared>, Json(body): Json<Value>) -> Response {
    match omapaper_mcp::handle(&app.host, &body) {
        Some(reply) => Json(reply).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}

// ---- WebSocket ------------------------------------------------------------

#[derive(Deserialize)]
struct WsQuery {
    canvas_id: Option<String>,
    #[serde(rename = "canvasId")]
    canvas_id_camel: Option<String>,
}

async fn ws(
    ws: WebSocketUpgrade,
    State(app): State<Shared>,
    Query(query): Query<WsQuery>,
) -> Response {
    let canvas_id = query
        .canvas_id
        .or(query.canvas_id_camel)
        .or_else(|| {
            app.host
                .store
                .list_canvases()
                .ok()
                .and_then(|c| c.first().map(|c| c.id.clone()))
        })
        .unwrap_or_else(|| "default".into());
    ws.on_upgrade(move |socket| socket_loop(app, socket, canvas_id))
}

async fn socket_loop(app: Shared, socket: WebSocket, canvas_id: String) {
    use omapaper_mcp::presence::Presence;
    let (mut sender, mut receiver) = socket.split();
    let mut rx = app.live.subscribe();
    let mut peer_id: Option<String> = None;

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(value) => {
                        let event_canvas = value.get("canvasId").and_then(Value::as_str);
                        let relevant = matches!(event_canvas, Some("*") | None) || event_canvas == Some(canvas_id.as_str());
                        if relevant
                            && sender
                                .send(Message::Text(value.to_string().into()))
                                .await
                                .is_err()
                        {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            incoming = receiver.next() => {
                let Some(Ok(message)) = incoming else { break };
                match message {
                    Message::Text(text) => {
                        let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                        match value.get("type").and_then(Value::as_str).unwrap_or("") {
                            "hello" => {
                                let id = value.get("peerId").and_then(Value::as_str)
                                    .map(str::to_string)
                                    .unwrap_or_else(new_id);
                                let name = value.get("name").and_then(Value::as_str)
                                    .unwrap_or("Guest")
                                    .to_string();
                                let kind = if value.get("kind").and_then(Value::as_str) == Some("agent") {
                                    PeerKind::Agent
                                } else {
                                    PeerKind::Human
                                };
                                peer_id = Some(id.clone());
                                app.live.set(&canvas_id, Peer {
                                    id,
                                    name: name.clone(),
                                    kind,
                                    color: color_for(&name),
                                    frame_id: None,
                                    x: 0.0,
                                    y: 0.0,
                                    status: None,
                                    selection: Vec::new(),
                                    updated_at: now_ms(),
                                });
                            }
                            "cursor" => {
                                let Some(id) = peer_id.clone() else { continue };
                                let existing = app.live.list(&canvas_id)
                                    .into_iter()
                                    .find(|p| p.id == id);
                                let Some(mut peer) = existing else { continue };
                                peer.x = value.get("x").and_then(Value::as_f64).unwrap_or(peer.x);
                                peer.y = value.get("y").and_then(Value::as_f64).unwrap_or(peer.y);
                                peer.frame_id = value.get("frameId").and_then(Value::as_str).map(str::to_string);
                                peer.updated_at = now_ms();
                                app.live.set(&canvas_id, peer);
                            }
                            "status" => {
                                let Some(id) = peer_id.clone() else { continue };
                                let existing = app.live.list(&canvas_id)
                                    .into_iter()
                                    .find(|p| p.id == id);
                                let Some(mut peer) = existing else { continue };
                                peer.status = value.get("status").and_then(Value::as_str).map(str::to_string);
                                peer.updated_at = now_ms();
                                app.live.set(&canvas_id, peer);
                            }
                            _ => {}
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        }
    }

    if let Some(peer_id) = peer_id {
        app.live.remove_peer(&canvas_id, &peer_id);
    }
}
