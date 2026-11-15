//! `omapaper-mcp` — the stdio entry point an agent host launches.
//!
//! One line of JSON-RPC in, one line out, exactly as the MCP stdio transport
//! expects. Logs go to stderr so stdout stays a clean protocol stream.

use omapaper_mcp::{handle_line, Host};
use std::io::{self, BufRead, Write};

fn main() {
    let host = Host::local();
    eprintln!(
        "omapaper-mcp {} — local canvas at {} (renderer: {})",
        omapaper_core::VERSION,
        host.data_dir.display(),
        host.renderer.name().unwrap_or_else(|| "none".into())
    );
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(reply) = handle_line(&host, line) {
            if writeln!(stdout, "{reply}").is_err() {
                break;
            }
            let _ = stdout.flush();
        }
    }
}
