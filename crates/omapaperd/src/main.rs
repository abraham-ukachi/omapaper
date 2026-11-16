//! `omapaperd` — the local OmaPaper daemon.
//!
//! Binds to loopback, serves the canvas UI, the REST API, the WebSocket and
//! the MCP endpoint, and follows Omarchy's current theme while it runs.

mod api;
mod live;
mod workflow;

use live::Live;
use omapaper_core::{theme::Theme, Store};
use omapaper_mcp::Host;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_PORT: u16 = 4310;

struct Args {
    host: String,
    port: u16,
    data_dir: PathBuf,
    db: Option<PathBuf>,
    help: bool,
    version: bool,
}

impl Args {
    fn parse() -> Args {
        let mut args = Args {
            host: std::env::var("OMAPPAPER_HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            port: DEFAULT_PORT,
            data_dir: default_data_dir(),
            db: None,
            help: false,
            version: false,
        };
        let mut argv = std::env::args().skip(1);
        while let Some(arg) = argv.next() {
            let (key, inline) = match arg.split_once('=') {
                Some((key, value)) => (key.to_string(), Some(value.to_string())),
                None => (arg, None),
            };
            let mut value = || inline.clone().or_else(|| argv.next()).unwrap_or_default();
            match key.as_str() {
                "--host" => args.host = value(),
                "--port" | "-p" => {
                    args.port = value().parse().unwrap_or(DEFAULT_PORT);
                }
                "--data-dir" => args.data_dir = PathBuf::from(value()),
                "--db" => args.db = Some(PathBuf::from(value())),
                "--help" | "-h" => args.help = true,
                "--version" | "-V" => args.version = true,
                other => eprintln!("unknown option `{other}` (try --help)"),
            }
        }
        args
    }
}

fn default_data_dir() -> PathBuf {
    std::env::var_os("OMAPPAPER_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Store::default_path()
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        })
}

fn help() {
    println!(
        "omapaperd {version} — local design canvas for humans and agents

USAGE
  omapaperd [--host 127.0.0.1] [--port {port}] [--data-dir DIR] [--db FILE]

OPTIONS
  --host <ADDR>      Address to bind (default 127.0.0.1)
  -p, --port <PORT>  Port (default {port})
  --data-dir <DIR>   Where the database, renders and exports live
  --db <FILE>        Use a specific database file
  -V, --version      Print the version
  -h, --help         This text

ENDPOINTS
  http://127.0.0.1:{port}/          the canvas
  http://127.0.0.1:{port}/mcp       MCP over HTTP (JSON-RPC)
  http://127.0.0.1:{port}/health    health and version
  ws://127.0.0.1:{port}/ws          presence, cursors, live edits
",
        version = omapaper_core::VERSION,
        port = DEFAULT_PORT
    );
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if args.help {
        help();
        return;
    }
    if args.version {
        println!("omapaperd {}", omapaper_core::VERSION);
        return;
    }

    std::fs::create_dir_all(&args.data_dir).expect("data directory");
    let plugins_dir = args.data_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("plugins directory");
    let db_path = args
        .db
        .clone()
        .unwrap_or_else(|| args.data_dir.join("omapaper.db"));
    let store = Store::open(&db_path).unwrap_or_else(|error| {
        panic!("cannot open {}: {error}", db_path.display());
    });
    let live = Live::new();
    let host = Host::with_parts(store, &args.data_dir, live.clone(), live.clone());
    let renderer = host.renderer.name();

    let app = Arc::new(api::App {
        host,
        live: live.clone(),
        data_dir: args.data_dir.clone(),
        plugins_dir: plugins_dir.clone(),
        port: args.port,
    });

    // Follow the Omarchy theme: a switch in the bar restyles every open tab.
    {
        let live = live.clone();
        tokio::spawn(async move {
            let mut last = Theme::live_omarchy();
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let current = Theme::live_omarchy();
                let changed = match (&last, &current) {
                    (Some(a), Some(b)) => {
                        a.name != b.name || a.accent != b.accent || a.background != b.background
                    }
                    (None, Some(_)) => true,
                    _ => false,
                };
                if changed {
                    if let Some(theme) = &current {
                        live.broadcast_theme(theme);
                    }
                }
                last = current;
            }
        });
    }

    // Scheduled automations: one tick a minute, queueing cards that are due.
    {
        let app = app.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                workflow::scheduled_tick(&app);
            }
        });
    }

    // Forget tabs that closed without saying goodbye.
    {
        let live = live.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(20)).await;
                live.prune();
            }
        });
    }

    let listener = tokio::net::TcpListener::bind((args.host.as_str(), args.port))
        .await
        .unwrap_or_else(|error| panic!("cannot bind {}:{}: {error}", args.host, args.port));

    let theme = Theme::live_omarchy();
    let theme_name = theme
        .as_ref()
        .map(|t| t.name.clone())
        .unwrap_or_else(|| "built-in".into());
    println!("OmaPaper {}", omapaper_core::VERSION);
    println!("  canvas   http://{}:{}/", args.host, args.port);
    println!("  mcp      http://{}:{}/mcp", args.host, args.port);
    println!("  storage  {}", db_path.display());
    println!("  theme    {theme_name}");
    println!(
        "  render   {}",
        renderer.unwrap_or_else(|| "none (install chromium for screenshots)".into())
    );

    axum::serve(listener, api::router(app))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server");
}
