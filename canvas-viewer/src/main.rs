//! `omapaper-canvas` — the native window the canvas opens in.
//!
//! GTK 3 + WebKitGTK: the same HTML/CSS/JS the daemon serves, wrapped in an
//! Omarchy window. Anything WebKit cannot do is still available at the same
//! URL in a browser, because the canvas is just a web page.

use gtk::prelude::*;
use webkit2gtk::{SettingsExt, WebView, WebViewExt};

const DEFAULT_URL: &str = "http://127.0.0.1:4310/";

struct Args {
    url: String,
    devtools: bool,
    width: i32,
    height: i32,
}

impl Args {
    fn parse() -> Args {
        let mut args = Args {
            url: DEFAULT_URL.into(),
            devtools: false,
            width: 1440,
            height: 900,
        };
        let mut argv = std::env::args().skip(1);
        while let Some(arg) = argv.next() {
            let (key, inline) = match arg.split_once('=') {
                Some((key, value)) => (key.to_string(), Some(value.to_string())),
                None => (arg, None),
            };
            let mut value = || inline.clone().or_else(|| argv.next()).unwrap_or_default();
            match key.as_str() {
                "--url" | "-u" => args.url = value(),
                "--width" => args.width = value().parse().unwrap_or(1440),
                "--height" => args.height = value().parse().unwrap_or(900),
                "--devtools" => args.devtools = true,
                "--help" | "-h" => {
                    println!(
                        "omapaper-canvas {version} — the OmaPaper canvas window\n\n\
                         USAGE\n    omapaper-canvas [--url <url>] [--width <px>] [--height <px>] [--devtools]\n",
                        version = env!("CARGO_PKG_VERSION")
                    );
                    std::process::exit(0);
                }
                "--version" | "-V" => {
                    println!("omapaper-canvas {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                other => eprintln!("unknown option `{other}` (try --help)"),
            }
        }
        args
    }
}

fn main() {
    let args = Args::parse();

    if gtk::init().is_err() {
        eprintln!("omapaper-canvas: could not start GTK (is a display available?)");
        std::process::exit(1);
    }

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("OmaPaper");
    window.set_default_size(args.width, args.height);
    window.set_icon_name(Some("omapaper"));

    let view = WebView::new();
    if let Some(settings) = WebViewExt::settings(&view) {
        settings.set_enable_developer_extras(args.devtools);
        settings.set_enable_smooth_scrolling(true);
        settings.set_enable_page_cache(false);
    }
    window.add(&view);
    window.show_all();
    // Load after the window is realized so the first paint is not dropped.
    view.load_uri(&args.url);
    window.connect_destroy(|_| gtk::main_quit());
    view.connect_close(|_| gtk::main_quit());

    gtk::main();
}
