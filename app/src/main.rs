//! `omapaper` — the cockpit.
//!
//! A small Slint window painted with Omarchy's live theme. It makes sure the
//! daemon is running, lists your canvases, creates new ones and hands a canvas
//! to the WebKit viewer (or your browser).

use omapaper_core::model::now_ms;
use omapaper_core::theme::Theme;
use serde_json::Value;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

slint::include_modules!();

const DEFAULT_PORT: u16 = 4310;

struct Args {
    base: String,
    port: u16,
    data_dir: Option<PathBuf>,
    canvas: Option<String>,
    no_daemon: bool,
    browser: bool,
}

impl Args {
    fn parse() -> Args {
        let mut args = Args {
            base: String::new(),
            port: DEFAULT_PORT,
            data_dir: None,
            canvas: None,
            no_daemon: false,
            browser: false,
        };
        let mut server = None;
        let mut argv = std::env::args().skip(1);
        while let Some(arg) = argv.next() {
            let (key, inline) = match arg.split_once('=') {
                Some((key, value)) => (key.to_string(), Some(value.to_string())),
                None => (arg, None),
            };
            let mut value = || inline.clone().or_else(|| argv.next()).unwrap_or_default();
            match key.as_str() {
                "--server" | "--url" => server = Some(value()),
                "--port" | "-p" => args.port = value().parse().unwrap_or(DEFAULT_PORT),
                "--data-dir" => args.data_dir = Some(PathBuf::from(value())),
                "--canvas" => args.canvas = Some(value()),
                "--no-daemon" => args.no_daemon = true,
                "--browser" => args.browser = true,
                "--help" | "-h" => {
                    println!("{}", help());
                    std::process::exit(0);
                }
                "--version" | "-V" => {
                    println!("omapaper {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                other => eprintln!("unknown option `{other}` (try --help)"),
            }
        }
        args.base = server
            .unwrap_or_else(|| format!("http://127.0.0.1:{}", args.port))
            .trim_end_matches('/')
            .to_string();
        args
    }

    fn is_local(&self) -> bool {
        self.base.contains("127.0.0.1") || self.base.contains("localhost")
    }
}

fn help() -> String {
    format!(
        "omapaper {version} — the OmaPaper cockpit

USAGE
    omapaper [options]

OPTIONS
    --server <url>    daemon base URL (default http://127.0.0.1:{DEFAULT_PORT})
    --port, -p <n>    daemon port when no --server is given
    --data-dir <dir>  data directory to hand the daemon
    --canvas <id>     open this canvas on start
    --no-daemon       never start omapaperd; only talk to a running one
    --browser         open canvases in the default browser instead of WebKit
    --help, -h        show this help
    --version, -V     print the version

The canvas itself is served at <url>/ and is fully usable in any browser.
",
        version = env!("CARGO_PKG_VERSION"),
        DEFAULT_PORT = DEFAULT_PORT,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.is_local() && !args.no_daemon {
        ensure_daemon(&args);
    }

    let ui = AppWindow::new()?;
    apply_theme(&ui);
    ui.set_endpoint(args.base.clone().into());

    // Follow Omarchy theme changes live.
    {
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let mut last = Theme::live_omarchy();
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let current = Theme::live_omarchy();
                let changed = match (&last, &current) {
                    (Some(a), Some(b)) => {
                        a.name != b.name || a.accent != b.accent || a.background != b.background
                    }
                    (None, Some(_)) => true,
                    _ => false,
                };
                if changed {
                    let weak = weak.clone();
                    let _ = weak.upgrade_in_event_loop(|ui| apply_theme(&ui));
                }
                last = current;
            }
        });
    }

    let base = args.base.clone();
    let weak = ui.as_weak();
    ui.on_refresh(move || {
        let weak = weak.clone();
        let base = base.clone();
        std::thread::spawn(move || refresh(&weak, &base));
    });

    let base = args.base.clone();
    let weak = ui.as_weak();
    ui.on_create(move |name| {
        let name = name.to_string();
        if name.trim().is_empty() {
            return;
        }
        let weak = weak.clone();
        let base = base.clone();
        std::thread::spawn(move || create_canvas(&weak, &base, name.trim()));
    });

    let base = args.base.clone();
    let browser = args.browser;
    let weak_open = ui.as_weak();
    ui.on_open_canvas(move |id| {
        open_url(&base, Some(id.as_str()), browser);
        let weak = weak_open.clone();
        let _ = weak.upgrade_in_event_loop(|ui| {
            let _ = ui.hide();
        });
        let _ = slint::quit_event_loop();
    });

    let base = args.base.clone();
    let weak_delete = ui.as_weak();
    ui.on_delete_canvas(move |id| {
        let id = id.to_string();
        let weak = weak_delete.clone();
        let base = base.clone();
        std::thread::spawn(move || {
            let _ = agent().delete(&format!("{base}/api/canvases/{id}")).call();
            refresh(&weak, &base);
        });
    });

    let base = args.base.clone();
    ui.on_open_browser(move || open_url(&base, None, true));

    // Poll: the daemon may start late, and an agent may add frames at any time.
    {
        let weak = ui.as_weak();
        let base = args.base.clone();
        std::thread::spawn(move || loop {
            refresh(&weak, &base);
            std::thread::sleep(Duration::from_secs(2));
        });
    }

    if let Some(canvas) = args.canvas.clone() {
        let weak = ui.as_weak();
        let base = args.base.clone();
        std::thread::spawn(move || {
            wait_for_daemon(&base, 40);
            open_url(&base, Some(&canvas), false);
            // Show it in the cockpit too.
            let _ = weak.upgrade_in_event_loop(|ui| {
                ui.set_selected(0);
            });
        });
    }

    ui.run()?;
    Ok(())
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(400))
        .timeout(Duration::from_secs(4))
        .build()
}

/// Starts `omapaperd` beside this binary (or on PATH) and waits for it.
fn ensure_daemon(args: &Args) {
    if agent().get(&format!("{}/health", args.base)).call().is_ok() {
        return;
    }

    let Some(daemon) = which("omapaperd") else {
        eprintln!("omapaperd was not found beside this binary or on PATH");
        return;
    };
    let mut command = Command::new(daemon);
    command
        .arg("--port")
        .arg(args.port.to_string())
        .env(
            "OMAPPAPER_DATA_DIR",
            args.data_dir.clone().unwrap_or_else(default_data_dir),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match command.spawn() {
        Ok(child) => eprintln!("started omapaperd (pid {})", child.id()),
        Err(error) => {
            eprintln!("could not start omapaperd: {error}");
            return;
        }
    }
    wait_for_daemon(&args.base, 50);
}

fn wait_for_daemon(base: &str, tenths: usize) {
    for _ in 0..tenths {
        if agent().get(&format!("{base}/health")).call().is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    eprintln!("omapaperd did not answer at {base}");
}

fn default_data_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into()))
        .join(".local/share/omapaper")
}

/// Paints the window with Omarchy's current theme.
fn apply_theme(ui: &AppWindow) {
    let theme = Theme::live_omarchy().unwrap_or_default();
    let palette = ui.global::<Palette>();
    let color = |rgb: omapaper_core::theme::Rgb| slint::Color::from_argb_encoded(rgb.argb());
    palette.set_bg(color(theme.bg()));
    palette.set_fg(color(theme.fg()));
    palette.set_accent(color(theme.accent_rgb()));
    palette.set_surface(color(theme.surface()));
    palette.set_hover(color(theme.hover()));
    palette.set_line(color(theme.line()));
    palette.set_raised(color(theme.raised()));
    palette.set_muted(color(theme.muted_rgb()));
    palette.set_ink_on_accent(color(theme.accent_rgb().ink_on(&theme)));
    palette.set_theme_name(theme.name.clone().into());
    palette.set_dark(theme.dark);
}

/// One poll: read `/api/info` and `/api/canvases`, then repaint the list.
fn refresh(weak: &slint::Weak<AppWindow>, base: &str) {
    let info: Option<Value> = agent()
        .get(&format!("{base}/api/info"))
        .call()
        .ok()
        .and_then(|response| response.into_json().ok());
    let canvases: Option<Value> = agent()
        .get(&format!("{base}/api/canvases"))
        .call()
        .ok()
        .and_then(|response| response.into_json().ok());

    let online = info.is_some();
    let version = info
        .as_ref()
        .and_then(|info| info.get("version"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let items: Vec<CanvasItem> = canvases
        .as_ref()
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(canvas_item).collect())
        .unwrap_or_default();

    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.set_online(online);
        ui.set_version(version.into());
        ui.set_status(if online {
            "local daemon running".into()
        } else {
            "starting the daemon…".into()
        });
        ui.set_canvases(ModelRc::new(VecModel::from(items.clone())));
        if ui.get_selected() >= items.len() as i32 {
            ui.set_selected(if items.is_empty() {
                -1
            } else {
                items.len() as i32 - 1
            });
        }
    });
}

fn canvas_item(value: &Value) -> Option<CanvasItem> {
    let id = value.get("id")?.as_str()?;
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Untitled");
    let pages = value.get("pages").and_then(Value::as_u64).unwrap_or(0);
    let frames = value.get("frames").and_then(Value::as_u64).unwrap_or(0);
    let updated = value.get("updatedAt").and_then(Value::as_i64).unwrap_or(0);
    let meta = format!(
        "{} page{} · {} frame{} · {}",
        pages,
        if pages == 1 { "" } else { "s" },
        frames,
        if frames == 1 { "" } else { "s" },
        ago(updated)
    );
    Some(CanvasItem {
        id: SharedString::from(id),
        name: SharedString::from(name),
        meta: SharedString::from(meta),
    })
}

fn ago(then: i64) -> String {
    let delta = (now_ms() - then).max(0) / 1000;
    match delta {
        0..=45 => "just now".into(),
        46..=3599 => format!("{}m ago", delta / 60),
        3600..=86_399 => format!("{}h ago", delta / 3600),
        _ => format!("{}d ago", delta / 86_400),
    }
}

fn create_canvas(weak: &slint::Weak<AppWindow>, base: &str, name: &str) {
    let created: Option<Value> = agent()
        .post(&format!("{base}/api/canvases"))
        .send_json(serde_json::json!({ "name": name }))
        .ok()
        .and_then(|response| response.into_json().ok());
    let id = created
        .as_ref()
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);

    refresh(weak, base);

    if let Some(id) = id {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            for index in 0..ui.get_canvases().row_count() {
                if ui.get_canvases().row_data(index).map(|row| row.id) == Some(id.clone().into()) {
                    ui.set_selected(index as i32);
                    break;
                }
            }
        });
    }
}

/// Opens a canvas in the WebKit viewer, falling back to the default browser.
fn open_url(base: &str, canvas: Option<&str>, browser: bool) {
    let url = match canvas {
        Some(id) => format!("{base}/?canvas={id}"),
        None => base.to_string(),
    };
    if !browser {
        if let Some(viewer) = which("omapaper-canvas") {
            let spawned = Command::new(viewer)
                .arg("--url")
                .arg(&url)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            if spawned.is_ok() {
                return;
            }
        }
    }
    let _ = Command::new("xdg-open")
        .arg(&url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Looks beside this executable first, then on `PATH`.
fn which(name: &str) -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        let beside = exe.with_file_name(name);
        if is_executable(&beside) {
            return Some(beside);
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}
