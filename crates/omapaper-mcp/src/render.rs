//! Screenshots and PDFs, through the browser already on the machine.
//!
//! Frames are HTML, so the only honest renderer is a browser engine. OmaPaper
//! shells out to Chromium in headless mode (the same engine Omarchy ships with
//! its Chromium) and treats its absence as a clear, non-fatal error: every
//! other tool keeps working without it.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("no Chromium found (install chromium, or set OMAPPAPER_CHROMIUM)")]
    NoBrowser,
    #[error("could not write the frame: {0}")]
    Io(#[from] std::io::Error),
    #[error("Chromium failed: {0}")]
    Browser(String),
    #[error("the frame did not render to an image")]
    Empty,
}

pub type Result<T> = std::result::Result<T, RenderError>;

/// One headless Chromium, found once at startup.
#[derive(Debug, Clone)]
pub struct Renderer {
    binary: Option<PathBuf>,
    work_dir: PathBuf,
}

impl Renderer {
    /// Finds Chromium: `$OMAPPAPER_CHROMIUM`, then the usual names on `PATH`.
    pub fn detect(work_dir: impl Into<PathBuf>) -> Renderer {
        let binary = std::env::var_os("OMAPPAPER_CHROMIUM")
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .or_else(|| {
                [
                    "chromium",
                    "chromium-browser",
                    "google-chrome-stable",
                    "google-chrome",
                ]
                .iter()
                .find_map(|name| which(name))
            });
        Renderer {
            binary,
            work_dir: work_dir.into(),
        }
    }

    pub fn available(&self) -> bool {
        self.binary.is_some()
    }

    pub fn name(&self) -> Option<String> {
        self.binary
            .as_ref()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
    }

    /// Renders a full HTML document to PNG bytes.
    pub fn screenshot(&self, html: &str, width: f64, height: f64, scale: f64) -> Result<Vec<u8>> {
        let binary = self.binary.as_ref().ok_or(RenderError::NoBrowser)?;
        std::fs::create_dir_all(&self.work_dir)?;
        let stamp = omapaper_core::new_id();
        let html_path = self.work_dir.join(format!("{stamp}.html"));
        let png_path = self.work_dir.join(format!("{stamp}.png"));
        std::fs::write(&html_path, html)?;

        let width = width.max(1.0).round() as u64;
        let height = height.max(1.0).round() as u64;
        let scale = scale.clamp(0.25, 3.0);
        let output = Command::new(binary)
            .args([
                "--headless=new",
                "--disable-gpu",
                "--hide-scrollbars",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-extensions",
                &format!("--force-device-scale-factor={scale}"),
                &format!("--window-size={width},{height}"),
                &format!("--screenshot={}", png_path.display()),
                &format!("file://{}", html_path.display()),
            ])
            .output()?;
        let _ = std::fs::remove_file(&html_path);
        if !png_path.is_file() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = std::fs::remove_file(&png_path);
            return Err(RenderError::Browser(
                stderr.lines().last().unwrap_or("no output").to_string(),
            ));
        }
        let bytes = std::fs::read(&png_path)?;
        let _ = std::fs::remove_file(&png_path);
        if bytes.is_empty() {
            return Err(RenderError::Empty);
        }
        Ok(bytes)
    }

    /// Prints a full HTML document to PDF bytes.
    pub fn pdf(&self, html: &str, width: f64, height: f64) -> Result<Vec<u8>> {
        let binary = self.binary.as_ref().ok_or(RenderError::NoBrowser)?;
        std::fs::create_dir_all(&self.work_dir)?;
        let stamp = omapaper_core::new_id();
        let html_path = self.work_dir.join(format!("{stamp}.html"));
        let pdf_path = self.work_dir.join(format!("{stamp}.pdf"));
        std::fs::write(&html_path, html)?;
        let width = width.max(1.0).round() as u64;
        let height = height.max(1.0).round() as u64;
        let output = Command::new(binary)
            .args([
                "--headless=new",
                "--disable-gpu",
                "--no-pdf-header-footer",
                "--no-first-run",
                &format!("--window-size={width},{height}"),
                &format!("--print-to-pdf={}", pdf_path.display()),
                &format!("file://{}", html_path.display()),
            ])
            .output()?;
        let _ = std::fs::remove_file(&html_path);
        if !pdf_path.is_file() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RenderError::Browser(
                stderr.lines().last().unwrap_or("no output").to_string(),
            ));
        }
        let bytes = std::fs::read(&pdf_path)?;
        let _ = std::fs::remove_file(&pdf_path);
        Ok(bytes)
    }

    /// Writes bytes to a file beneath `dir`, returning its path.
    pub fn write_export(&self, dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(name);
        std::fs::write(&path, bytes)?;
        Ok(path)
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Standard base64, so image content needs no dependency.
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(n >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[n as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"hello\n"), "aGVsbG8K");
    }

    #[test]
    fn missing_chromium_is_a_clear_error() {
        let mut renderer = Renderer::detect(std::env::temp_dir());
        renderer.binary = None;
        assert!(!renderer.available());
        assert!(matches!(
            renderer.screenshot("<p>x</p>", 10.0, 10.0, 1.0),
            Err(RenderError::NoBrowser)
        ));
    }
}
