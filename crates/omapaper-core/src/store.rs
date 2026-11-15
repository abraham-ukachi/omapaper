//! The one file OmaPaper keeps: a SQLite database under
//! `~/.local/share/omapaper/omapaper.db`.
//!
//! Canvases carry their pages, tokens, guides and decisions as JSON columns;
//! frames are rows of their own so an agent streaming HTML rewrites one frame,
//! never the whole document. Writes are synchronous and small — a local daemon
//! on one machine does not need a pool.

use crate::model::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found")]
    NotFound,
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// A handle to the database. Cheap to clone; safe to share across threads.
#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
    path: Option<PathBuf>,
}

impl Store {
    /// Opens (and migrates) the database at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Store> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Store {
            conn: Arc::new(Mutex::new(conn)),
            path: Some(path),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_memory() -> Result<Store> {
        let conn = Connection::open_in_memory()?;
        let store = Store {
            conn: Arc::new(Mutex::new(conn)),
            path: None,
        };
        store.migrate()?;
        Ok(store)
    }

    /// `~/.local/share/omapaper/omapaper.db`, honouring `$XDG_DATA_HOME`.
    pub fn default_path() -> PathBuf {
        if let Ok(data) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(data).join("omapaper/omapaper.db");
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".local/share/omapaper/omapaper.db")
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn migrate(&self) -> Result<()> {
        self.with(|conn| {
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS canvases (
                  id TEXT PRIMARY KEY,
                  name TEXT NOT NULL,
                  pages TEXT NOT NULL,
                  tokens TEXT NOT NULL,
                  token_themes TEXT NOT NULL,
                  guidelines TEXT NOT NULL,
                  decisions TEXT NOT NULL,
                  created_at INTEGER NOT NULL,
                  updated_at INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS frames (
                  id TEXT PRIMARY KEY,
                  canvas_id TEXT NOT NULL,
                  page_id TEXT NOT NULL,
                  name TEXT NOT NULL,
                  html TEXT NOT NULL,
                  x REAL NOT NULL,
                  y REAL NOT NULL,
                  width REAL NOT NULL,
                  height REAL NOT NULL,
                  background TEXT,
                  theme TEXT,
                  locked INTEGER NOT NULL DEFAULT 0,
                  approval TEXT NOT NULL DEFAULT 'draft',
                  approval_by TEXT,
                  revision INTEGER NOT NULL DEFAULT 1,
                  ord INTEGER NOT NULL DEFAULT 0,
                  created_at INTEGER NOT NULL,
                  updated_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS frames_canvas ON frames (canvas_id, page_id, ord);
                CREATE TABLE IF NOT EXISTS versions (
                  id TEXT PRIMARY KEY,
                  frame_id TEXT NOT NULL,
                  html TEXT NOT NULL,
                  width REAL NOT NULL,
                  height REAL NOT NULL,
                  label TEXT,
                  created_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS versions_frame ON versions (frame_id, created_at);
                CREATE TABLE IF NOT EXISTS comments (
                  id TEXT PRIMARY KEY,
                  canvas_id TEXT NOT NULL,
                  frame_id TEXT,
                  page_id TEXT,
                  element_id TEXT,
                  x REAL,
                  y REAL,
                  body TEXT NOT NULL,
                  author TEXT NOT NULL,
                  resolved INTEGER NOT NULL DEFAULT 0,
                  created_at INTEGER NOT NULL,
                  replies TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS comments_canvas ON comments (canvas_id, created_at);
                CREATE TABLE IF NOT EXISTS assets (
                  id TEXT PRIMARY KEY,
                  canvas_id TEXT NOT NULL,
                  mime TEXT NOT NULL,
                  bytes BLOB NOT NULL,
                  created_at INTEGER NOT NULL
                );
                "#,
            )?;
            Ok(())
        })
    }

    fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let guard = self
            .conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&guard)
    }

    // ---- canvases ---------------------------------------------------------

    pub fn list_canvases(&self) -> Result<Vec<CanvasSummary>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT c.id, c.name, c.pages, c.updated_at,
                        (SELECT COUNT(*) FROM frames f WHERE f.canvas_id = c.id)
                 FROM canvases c ORDER BY c.updated_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                let pages: String = row.get(2)?;
                let frames: i64 = row.get(4)?;
                Ok(CanvasSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    pages: serde_json::from_str::<Vec<Page>>(&pages)
                        .map(|p| p.len())
                        .unwrap_or(1),
                    frames: frames as usize,
                    updated_at: row.get(3)?,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn create_canvas(&self, name: &str) -> Result<Canvas> {
        let canvas = Canvas::new(name);
        self.with(|conn| {
            conn.execute(
                "INSERT INTO canvases (id, name, pages, tokens, token_themes, guidelines, decisions, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    canvas.id,
                    canvas.name,
                    serde_json::to_string(&canvas.pages)?,
                    serde_json::to_string(&canvas.tokens)?,
                    serde_json::to_string(&canvas.token_themes)?,
                    serde_json::to_string(&canvas.guidelines)?,
                    serde_json::to_string(&canvas.decisions)?,
                    canvas.created_at,
                    canvas.updated_at,
                ],
            )?;
            Ok(())
        })?;
        Ok(canvas)
    }

    pub fn get_canvas(&self, id: &str) -> Result<Option<Canvas>> {
        self.with(|conn| {
            let row = conn
                .query_row(
                    "SELECT id, name, pages, tokens, token_themes, guidelines, decisions, created_at, updated_at
                     FROM canvases WHERE id = ?1",
                    params![id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, i64>(7)?,
                            row.get::<_, i64>(8)?,
                        ))
                    },
                )
                .optional()?;
            let Some((id, name, pages, tokens, themes, guidelines, decisions, created, updated)) = row
            else {
                return Ok(None);
            };
            Ok(Some(Canvas {
                id,
                name,
                pages: serde_json::from_str(&pages).unwrap_or_default(),
                tokens: serde_json::from_str(&tokens).unwrap_or_default(),
                token_themes: serde_json::from_str(&themes).unwrap_or_default(),
                guidelines: serde_json::from_str(&guidelines).unwrap_or_default(),
                decisions: serde_json::from_str(&decisions).unwrap_or_default(),
                created_at: created,
                updated_at: updated,
            }))
        })
    }

    pub fn save_canvas(&self, canvas: &Canvas) -> Result<()> {
        let canvas = Canvas {
            updated_at: now_ms(),
            ..canvas.clone()
        };
        self.with(|conn| {
            conn.execute(
                "UPDATE canvases SET name = ?2, pages = ?3, tokens = ?4, token_themes = ?5,
                   guidelines = ?6, decisions = ?7, updated_at = ?8 WHERE id = ?1",
                params![
                    canvas.id,
                    canvas.name,
                    serde_json::to_string(&canvas.pages)?,
                    serde_json::to_string(&canvas.tokens)?,
                    serde_json::to_string(&canvas.token_themes)?,
                    serde_json::to_string(&canvas.guidelines)?,
                    serde_json::to_string(&canvas.decisions)?,
                    canvas.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn delete_canvas(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute("DELETE FROM canvases WHERE id = ?1", params![id])?;
            conn.execute("DELETE FROM versions WHERE frame_id IN (SELECT id FROM frames WHERE canvas_id = ?1)", params![id])?;
            conn.execute("DELETE FROM frames WHERE canvas_id = ?1", params![id])?;
            conn.execute("DELETE FROM comments WHERE canvas_id = ?1", params![id])?;
            conn.execute("DELETE FROM assets WHERE canvas_id = ?1", params![id])?;
            Ok(())
        })
    }

    pub fn touch_canvas(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE canvases SET updated_at = ?2 WHERE id = ?1",
                params![id, now_ms()],
            )?;
            Ok(())
        })
    }

    // ---- frames -----------------------------------------------------------

    pub fn frames(&self, canvas_id: &str) -> Result<Vec<Frame>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, canvas_id, page_id, name, html, x, y, width, height, background, theme,
                        locked, approval, approval_by, revision, ord, created_at, updated_at
                 FROM frames WHERE canvas_id = ?1 ORDER BY ord, created_at",
            )?;
            let rows = stmt.query_map(params![canvas_id], row_to_frame)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn get_frame(&self, id: &str) -> Result<Option<Frame>> {
        self.with(|conn| {
            Ok(conn
                .query_row(
                    "SELECT id, canvas_id, page_id, name, html, x, y, width, height, background, theme,
                            locked, approval, approval_by, revision, ord, created_at, updated_at
                     FROM frames WHERE id = ?1",
                    params![id],
                    row_to_frame,
                )
                .optional()?)
        })
    }

    pub fn insert_frame(&self, frame: &Frame) -> Result<()> {
        self.with(|conn| {
            let ord: i64 = conn.query_row(
                "SELECT COALESCE(MAX(ord), -1) + 1 FROM frames WHERE canvas_id = ?1",
                params![frame.canvas_id],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO frames (id, canvas_id, page_id, name, html, x, y, width, height, background,
                    theme, locked, approval, approval_by, revision, ord, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                params![
                    frame.id,
                    frame.canvas_id,
                    frame.page_id,
                    frame.name,
                    frame.html,
                    frame.x,
                    frame.y,
                    frame.width,
                    frame.height,
                    frame.background,
                    frame.theme,
                    if frame.locked { 1 } else { 0 },
                    approval_key(frame.approval),
                    frame.approval_by,
                    frame.revision,
                    ord,
                    frame.created_at,
                    frame.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn save_frame(&self, frame: &Frame) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE frames SET page_id = ?2, name = ?3, html = ?4, x = ?5, y = ?6, width = ?7,
                    height = ?8, background = ?9, theme = ?10, locked = ?11, approval = ?12,
                    approval_by = ?13, revision = ?14, ord = ?15, updated_at = ?16 WHERE id = ?1",
                params![
                    frame.id,
                    frame.page_id,
                    frame.name,
                    frame.html,
                    frame.x,
                    frame.y,
                    frame.width,
                    frame.height,
                    frame.background,
                    frame.theme,
                    if frame.locked { 1 } else { 0 },
                    approval_key(frame.approval),
                    frame.approval_by,
                    frame.revision,
                    frame.ord,
                    now_ms(),
                ],
            )?;
            Ok(())
        })
    }

    pub fn delete_frame(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute("DELETE FROM frames WHERE id = ?1", params![id])?;
            conn.execute("DELETE FROM versions WHERE frame_id = ?1", params![id])?;
            Ok(())
        })
    }

    pub fn frame_count(&self, canvas_id: &str) -> Result<usize> {
        self.with(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM frames WHERE canvas_id = ?1",
                params![canvas_id],
                |row| row.get(0),
            )?;
            Ok(count as usize)
        })
    }

    // ---- versions ---------------------------------------------------------

    pub fn push_version(&self, frame: &Frame, label: Option<&str>, keep: usize) -> Result<Version> {
        let version = Version {
            id: new_id(),
            frame_id: frame.id.clone(),
            html: frame.html.clone(),
            width: frame.width,
            height: frame.height,
            label: label.map(str::to_string),
            created_at: now_ms(),
        };
        self.with(|conn| {
            conn.execute(
                "INSERT INTO versions (id, frame_id, html, width, height, label, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    version.id,
                    version.frame_id,
                    version.html,
                    version.width,
                    version.height,
                    version.label,
                    version.created_at,
                ],
            )?;
            conn.execute(
                "DELETE FROM versions WHERE frame_id = ?1 AND id NOT IN (
                    SELECT id FROM versions WHERE frame_id = ?1 ORDER BY created_at DESC LIMIT ?2)",
                params![version.frame_id, keep as i64],
            )?;
            Ok(())
        })?;
        Ok(version)
    }

    pub fn versions(&self, frame_id: &str) -> Result<Vec<Version>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, frame_id, html, width, height, label, created_at FROM versions
                 WHERE frame_id = ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(params![frame_id], |row| {
                Ok(Version {
                    id: row.get(0)?,
                    frame_id: row.get(1)?,
                    html: row.get(2)?,
                    width: row.get(3)?,
                    height: row.get(4)?,
                    label: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn version(&self, id: &str) -> Result<Option<Version>> {
        self.with(|conn| {
            Ok(conn
                .query_row(
                    "SELECT id, frame_id, html, width, height, label, created_at FROM versions WHERE id = ?1",
                    params![id],
                    |row| {
                        Ok(Version {
                            id: row.get(0)?,
                            frame_id: row.get(1)?,
                            html: row.get(2)?,
                            width: row.get(3)?,
                            height: row.get(4)?,
                            label: row.get(5)?,
                            created_at: row.get(6)?,
                        })
                    },
                )
                .optional()?)
        })
    }

    // ---- comments ---------------------------------------------------------

    pub fn comments(&self, canvas_id: &str) -> Result<Vec<Comment>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, canvas_id, frame_id, page_id, element_id, x, y, body, author, resolved,
                        created_at, replies
                 FROM comments WHERE canvas_id = ?1 ORDER BY created_at",
            )?;
            let rows = stmt.query_map(params![canvas_id], |row| {
                let replies: String = row.get(11)?;
                Ok(Comment {
                    id: row.get(0)?,
                    canvas_id: row.get(1)?,
                    frame_id: row.get(2)?,
                    page_id: row.get(3)?,
                    element_id: row.get(4)?,
                    x: row.get(5)?,
                    y: row.get(6)?,
                    body: row.get(7)?,
                    author: row.get(8)?,
                    resolved: row.get::<_, i64>(9)? != 0,
                    created_at: row.get(10)?,
                    replies: serde_json::from_str(&replies).unwrap_or_default(),
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn add_comment(&self, comment: &Comment) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO comments (id, canvas_id, frame_id, page_id, element_id, x, y, body,
                    author, resolved, created_at, replies)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    comment.id,
                    comment.canvas_id,
                    comment.frame_id,
                    comment.page_id,
                    comment.element_id,
                    comment.x,
                    comment.y,
                    comment.body,
                    comment.author,
                    if comment.resolved { 1 } else { 0 },
                    comment.created_at,
                    serde_json::to_string(&comment.replies)?,
                ],
            )?;
            Ok(())
        })
    }

    pub fn get_comment(&self, id: &str) -> Result<Option<Comment>> {
        self.with(|conn| {
            let row = conn
                .query_row(
                    "SELECT id, canvas_id, frame_id, page_id, element_id, x, y, body, author, resolved,
                            created_at, replies FROM comments WHERE id = ?1",
                    params![id],
                    |row| {
                        let replies: String = row.get(11)?;
                        Ok(Comment {
                            id: row.get(0)?,
                            canvas_id: row.get(1)?,
                            frame_id: row.get(2)?,
                            page_id: row.get(3)?,
                            element_id: row.get(4)?,
                            x: row.get(5)?,
                            y: row.get(6)?,
                            body: row.get(7)?,
                            author: row.get(8)?,
                            resolved: row.get::<_, i64>(9)? != 0,
                            created_at: row.get(10)?,
                            replies: serde_json::from_str(&replies).unwrap_or_default(),
                        })
                    },
                )
                .optional()?;
            Ok(row)
        })
    }

    pub fn save_comment(&self, comment: &Comment) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE comments SET body = ?2, resolved = ?3, replies = ?4 WHERE id = ?1",
                params![
                    comment.id,
                    comment.body,
                    if comment.resolved { 1 } else { 0 },
                    serde_json::to_string(&comment.replies)?,
                ],
            )?;
            Ok(())
        })
    }

    // ---- assets -----------------------------------------------------------

    pub fn put_asset(&self, canvas_id: &str, mime: &str, bytes: &[u8]) -> Result<String> {
        let id = new_id();
        self.with(|conn| {
            conn.execute(
                "INSERT INTO assets (id, canvas_id, mime, bytes, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, canvas_id, mime, bytes, now_ms()],
            )?;
            Ok(())
        })?;
        Ok(id)
    }

    pub fn get_asset(&self, id: &str) -> Result<Option<(String, Vec<u8>)>> {
        self.with(|conn| {
            Ok(conn
                .query_row(
                    "SELECT mime, bytes FROM assets WHERE id = ?1",
                    params![id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
                )
                .optional()?)
        })
    }
}

fn approval_key(approval: Approval) -> &'static str {
    match approval {
        Approval::Draft => "draft",
        Approval::ReadyForDev => "ready_for_dev",
        Approval::NeedsReview => "needs_review",
    }
}

fn approval_from(value: &str) -> Approval {
    match value {
        "ready_for_dev" => Approval::ReadyForDev,
        "needs_review" => Approval::NeedsReview,
        _ => Approval::Draft,
    }
}

fn row_to_frame(row: &rusqlite::Row<'_>) -> rusqlite::Result<Frame> {
    let approval: String = row.get(12)?;
    Ok(Frame {
        id: row.get(0)?,
        canvas_id: row.get(1)?,
        page_id: row.get(2)?,
        name: row.get(3)?,
        html: row.get(4)?,
        x: row.get(5)?,
        y: row.get(6)?,
        width: row.get(7)?,
        height: row.get(8)?,
        background: row.get(9)?,
        theme: row.get(10)?,
        locked: row.get::<_, i64>(11)? != 0,
        approval: approval_from(&approval),
        approval_by: row.get(13)?,
        revision: row.get(14)?,
        ord: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::open_memory().expect("memory store")
    }

    #[test]
    fn canvases_round_trip() {
        let store = store();
        let canvas = store.create_canvas("Website").unwrap();
        let loaded = store.get_canvas(&canvas.id).unwrap().unwrap();
        assert_eq!(loaded.name, "Website");
        assert_eq!(loaded.pages.len(), 1);
        let summaries = store.list_canvases().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "Website");
        assert_eq!(summaries[0].frames, 0);
    }

    #[test]
    fn frames_are_ordered_and_deletable() {
        let store = store();
        let canvas = store.create_canvas("Canvas").unwrap();
        let page = canvas.pages[0].id.clone();
        let a = Frame::new(&canvas.id, &page, "Hero");
        let b = Frame::new(&canvas.id, &page, "Pricing");
        store.insert_frame(&a).unwrap();
        store.insert_frame(&b).unwrap();
        let frames = store.frames(&canvas.id).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].name, "Hero");
        assert_eq!(frames[1].ord, 1);
        store.delete_frame(&a.id).unwrap();
        assert_eq!(store.frames(&canvas.id).unwrap().len(), 1);
    }

    #[test]
    fn versions_keep_the_latest_n() {
        let store = store();
        let canvas = store.create_canvas("Canvas").unwrap();
        let mut frame = Frame::new(&canvas.id, &canvas.pages[0].id, "Frame");
        store.insert_frame(&frame).unwrap();
        for i in 0..5 {
            frame.html = format!("<p>{i}</p>");
            store
                .push_version(&frame, Some(&format!("v{i}")), 3)
                .unwrap();
        }
        let versions = store.versions(&frame.id).unwrap();
        assert_eq!(versions.len(), 3);
    }

    #[test]
    fn comments_round_trip_with_replies() {
        let store = store();
        let canvas = store.create_canvas("Canvas").unwrap();
        let mut comment = Comment {
            id: new_id(),
            canvas_id: canvas.id.clone(),
            frame_id: None,
            page_id: None,
            element_id: Some("hero-title".into()),
            x: None,
            y: None,
            body: "Make this bigger".into(),
            author: "Abraham".into(),
            resolved: false,
            created_at: now_ms(),
            replies: vec![],
        };
        store.add_comment(&comment).unwrap();
        comment.replies.push(Reply {
            id: new_id(),
            author: "Claude".into(),
            body: "Done".into(),
            created_at: now_ms(),
        });
        comment.resolved = true;
        store.save_comment(&comment).unwrap();
        let loaded = store.get_comment(&comment.id).unwrap().unwrap();
        assert!(loaded.resolved);
        assert_eq!(loaded.replies.len(), 1);
    }

    #[test]
    fn assets_round_trip() {
        let store = store();
        let canvas = store.create_canvas("Canvas").unwrap();
        let id = store.put_asset(&canvas.id, "image/png", b"png").unwrap();
        let (mime, bytes) = store.get_asset(&id).unwrap().unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(bytes, b"png");
    }
}
