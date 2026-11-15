//! Omarchy themes, painted the Omacalc way, plus the ab-nextjs-theme palettes.
//!
//! A theme is three signature colours — background, foreground and accent —
//! and, when Omarchy is running, the full terminal palette. Every other colour
//! in the UI is a mix of the first two, exactly like Omacalc's `CalcButton.qml`:
//!
//! | No. | Role | Mix of the foreground into the background |
//! |:----|:-----|:------------------------------------------|
//! | 1 | surface (cards, keys) | 5%, with a 13% border |
//! | 2 | raised / active key | 16% |
//! | 3 | divider | 16% |
//! | 4 | muted text | 50% |
//! | 5 | primary (`OK`, `=`) | inverted: foreground fill, background text |

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// An RGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const WHITE: Rgb = Rgb(255, 255, 255);
    pub const BLACK: Rgb = Rgb(0, 0, 0);

    /// Parses `#rrggbb` (case-insensitive).
    pub fn hex(s: &str) -> Option<Rgb> {
        let s = s.trim().trim_start_matches('#');
        if s.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(s, 16).ok()?;
        Some(Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }

    /// Mixes `tint` into `self` by `amount` (0.0 to 1.0), like Omacalc's `mixColors`.
    pub fn mix(self, tint: Rgb, amount: f32) -> Rgb {
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
        Rgb(m(self.0, tint.0), m(self.1, tint.1), m(self.2, tint.2))
    }

    pub fn hex_string(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// `0xAARRGGBB`, the format Slint's `Color::from_argb_encoded` takes.
    pub fn argb(self) -> u32 {
        0xff00_0000 | (self.0 as u32) << 16 | (self.1 as u32) << 8 | self.2 as u32
    }

    /// Relative luminance (WCAG), used to pick the ink on top of a colour.
    pub fn luminance(self) -> f32 {
        let channel = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.0) + 0.7152 * channel(self.1) + 0.0722 * channel(self.2)
    }

    pub fn is_dark(self) -> bool {
        self.luminance() < 0.35
    }

    /// The ink to put on top of this colour, theme fore/back aware.
    pub fn ink_on(self, theme: &Theme) -> Rgb {
        if self.is_dark() {
            theme.fg()
        } else {
            theme.bg()
        }
    }
}

/// The Omarchy terminal palette, when `colors.toml` provides one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub red: Option<String>,
    pub yellow: Option<String>,
    pub orange: Option<String>,
    pub green: Option<String>,
    pub cyan: Option<String>,
    pub blue: Option<String>,
    pub magenta: Option<String>,
    pub bright_red: Option<String>,
    pub bright_yellow: Option<String>,
    pub bright_green: Option<String>,
    pub bright_cyan: Option<String>,
    pub bright_blue: Option<String>,
    pub bright_magenta: Option<String>,
}

/// One theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub name: String,
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub muted: Option<String>,
    pub selection: Option<String>,
    pub dark: bool,
    pub palette: Palette,
    /// `omarchy` (live `colors.toml`), `built-in` or `ab-nextjs-theme`.
    pub source: String,
}

impl Default for Theme {
    fn default() -> Theme {
        Theme::named("Tokyo Night")
    }
}

impl Theme {
    /// Every theme that ships with Omarchy, sorted by name. Only the three
    /// signature colours are known; the palette arrives with the live theme.
    pub fn all() -> Vec<Theme> {
        BUILT_IN
            .iter()
            .map(|(name, bg, fg, accent, dark)| Theme {
                name: (*name).to_string(),
                background: (*bg).to_string(),
                foreground: (*fg).to_string(),
                accent: (*accent).to_string(),
                muted: None,
                selection: None,
                dark: *dark,
                palette: Palette::default(),
                source: "built-in".into(),
            })
            .collect()
    }

    /// Finds a theme by name, falling back to Tokyo Night (Omarchy's default).
    pub fn named(name: &str) -> Theme {
        Theme::all()
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| {
                let (name, bg, fg, accent, dark) = BUILT_IN[20];
                Theme {
                    name: name.to_string(),
                    background: bg.to_string(),
                    foreground: fg.to_string(),
                    accent: accent.to_string(),
                    muted: None,
                    selection: None,
                    dark,
                    palette: Palette::default(),
                    source: "built-in".into(),
                }
            })
    }

    /// The ab-nextjs-theme Material scheme (`light` or `dark`), as a theme.
    pub fn ab_nextjs(scheme: &str) -> Theme {
        let rows: &[(&str, &str)] = if scheme.eq_ignore_ascii_case("dark") {
            AB_DARK
        } else {
            AB_LIGHT
        };
        let get = |key: &str| {
            rows.iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
                .unwrap_or_default()
        };
        Theme {
            name: format!(
                "ab-nextjs · {}",
                if scheme.eq_ignore_ascii_case("dark") {
                    "dark"
                } else {
                    "light"
                }
            ),
            background: get("background"),
            foreground: get("onBackground"),
            accent: get("primary"),
            muted: Some(get("onSurfaceVariant")),
            selection: Some(get("surfaceVariant")),
            dark: scheme.eq_ignore_ascii_case("dark"),
            palette: Palette {
                red: Some(get("error")),
                yellow: Some(get("secondary")),
                green: Some(get("tertiary")),
                cyan: Some(get("tertiary")),
                blue: Some(get("primary")),
                magenta: Some(get("secondary")),
                ..Palette::default()
            },
            source: "ab-nextjs-theme".into(),
        }
    }

    /// Both ab-nextjs-theme schemes, light first.
    pub fn ab_nextjs_all() -> Vec<Theme> {
        vec![Theme::ab_nextjs("light"), Theme::ab_nextjs("dark")]
    }

    /// Every theme OmaPaper can paint: the live Omarchy theme first, then the
    /// 23 built-ins, then the two ab-nextjs-theme schemes.
    pub fn catalogue(live: Option<Theme>) -> Vec<Theme> {
        let mut themes = Vec::new();
        if let Some(live) = live {
            themes.push(live);
        }
        for theme in Theme::all() {
            if !themes.iter().any(|t| t.name == theme.name) {
                themes.push(theme);
            }
        }
        themes.extend(Theme::ab_nextjs_all());
        themes
    }

    /// Reads Omarchy's current theme (`~/.local/state/omarchy/current/theme`).
    pub fn live_omarchy() -> Option<Theme> {
        let current = omarchy_current_dir()?;
        Theme::from_omarchy_dir(&current)
    }

    /// Reads `<dir>/colors.toml`. The name comes from `theme.name` beside the
    /// symlink, else from the directory name.
    pub fn from_omarchy_dir(dir: &Path) -> Option<Theme> {
        let colors_path = dir.join("colors.toml");
        let text = std::fs::read_to_string(&colors_path).ok()?;
        let doc: toml::Value = toml::from_str(&text).ok()?;
        let get = |key: &str| {
            doc.get(key)
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        };
        let background = get("background")?;
        let foreground = get("foreground")?;
        let accent = get("accent").unwrap_or_else(|| foreground.clone());
        let mode = get("mode").unwrap_or_else(|| {
            if Rgb::hex(&background).map(Rgb::is_dark).unwrap_or(true) {
                "dark".into()
            } else {
                "light".into()
            }
        });
        let name = std::fs::read_to_string(dir.join("..").join("theme.name"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                std::fs::canonicalize(dir)
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            })
            .unwrap_or_else(|| "Omarchy".into());

        Some(Theme {
            name: title_case(&name),
            background,
            foreground,
            accent,
            muted: get("muted"),
            selection: get("selection"),
            dark: mode == "dark",
            palette: Palette {
                red: get("red"),
                yellow: get("yellow"),
                orange: get("orange"),
                green: get("green"),
                cyan: get("cyan"),
                blue: get("blue"),
                magenta: get("magenta"),
                bright_red: get("bright_red"),
                bright_yellow: get("bright_yellow"),
                bright_green: get("bright_green"),
                bright_cyan: get("bright_cyan"),
                bright_blue: get("bright_blue"),
                bright_magenta: get("bright_magenta"),
            },
            source: "omarchy".into(),
        })
    }

    pub fn bg(&self) -> Rgb {
        Rgb::hex(&self.background).unwrap_or(Rgb::BLACK)
    }

    pub fn fg(&self) -> Rgb {
        Rgb::hex(&self.foreground).unwrap_or(Rgb::WHITE)
    }

    pub fn accent_rgb(&self) -> Rgb {
        Rgb::hex(&self.accent).unwrap_or(self.fg())
    }

    /// 5% foreground: cards, keys.
    pub fn surface(&self) -> Rgb {
        self.bg().mix(self.fg(), 0.05)
    }

    /// 8% foreground: hover.
    pub fn hover(&self) -> Rgb {
        self.bg().mix(self.fg(), 0.08)
    }

    /// 13% foreground: borders.
    pub fn line(&self) -> Rgb {
        self.bg().mix(self.fg(), 0.13)
    }

    /// 16% foreground: raised surfaces, dividers.
    pub fn raised(&self) -> Rgb {
        self.bg().mix(self.fg(), 0.16)
    }

    /// 50% foreground: secondary text.
    pub fn muted_rgb(&self) -> Rgb {
        self.bg().mix(self.fg(), 0.5)
    }

    /// The CSS custom properties every OmaPaper surface reads.
    pub fn css_vars(&self) -> String {
        let mut out = String::from(":root{\n");
        let mut push = |name: &str, value: String| {
            out.push_str(&format!("  --op-{name}: {value};\n"));
        };
        push("bg", self.bg().hex_string());
        push("fg", self.fg().hex_string());
        push("accent", self.accent_rgb().hex_string());
        push("surface", self.surface().hex_string());
        push("hover", self.hover().hex_string());
        push("line", self.line().hex_string());
        push("raised", self.raised().hex_string());
        push("muted", self.muted_rgb().hex_string());
        push("ink-on-accent", self.accent_rgb().ink_on(self).hex_string());
        push("theme-name", json_string(&self.name));
        push("mode", if self.dark { "dark" } else { "light" }.to_string());

        let p = &self.palette;
        for (name, value) in [
            ("red", &p.red),
            ("yellow", &p.yellow),
            ("orange", &p.orange),
            ("green", &p.green),
            ("cyan", &p.cyan),
            ("blue", &p.blue),
            ("magenta", &p.magenta),
            ("bright-red", &p.bright_red),
            ("bright-yellow", &p.bright_yellow),
            ("bright-green", &p.bright_green),
            ("bright-cyan", &p.bright_cyan),
            ("bright-blue", &p.bright_blue),
            ("bright-magenta", &p.bright_magenta),
        ] {
            if let Some(value) = value {
                push(name, value.clone());
            }
        }
        out.push_str("}\n");
        out
    }
}

fn json_string(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

fn title_case(value: &str) -> String {
    value
        .split(['-', '_'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `~/.local/state/omarchy/current/theme`, honouring `$XDG_STATE_HOME`.
pub fn omarchy_current_dir() -> Option<PathBuf> {
    if let Ok(state) = std::env::var("XDG_STATE_HOME") {
        let dir = PathBuf::from(state).join("omarchy/current/theme");
        if dir.is_dir() {
            return Some(dir);
        }
    }
    let home = std::env::var_os("HOME")?;
    let dir = PathBuf::from(home).join(".local/state/omarchy/current/theme");
    dir.is_dir().then_some(dir)
}

/// The 23 themes Omarchy ships, sorted by name.
pub const BUILT_IN: &[(&str, &str, &str, &str, bool)] = &[
    ("Catppuccin", "#1e1e2e", "#cdd6f4", "#89b4fa", true),
    ("Catppuccin Latte", "#eff1f5", "#4c4f69", "#1e66f5", false),
    ("Ethereal", "#060B1E", "#ffcead", "#7d82d9", true),
    ("Everforest", "#2d353b", "#d3c6aa", "#7fbbb3", true),
    ("Flexoki Light", "#FFFCF0", "#100F0F", "#205EA6", false),
    ("Gruvbox", "#282828", "#d4be98", "#7daea3", true),
    ("Hackerman", "#0B0C16", "#ddf7ff", "#82FB9C", true),
    ("Jade Horse", "#142620", "#dfede5", "#8bcbb1", true),
    ("Kanagawa", "#1f1f28", "#dcd7ba", "#dcd7ba", true),
    ("Last Horizon", "#0c0b0c", "#FAFCFB", "#b59790", true),
    ("Lumon", "#16242d", "#d6e2ee", "#8bc9eb", true),
    ("Lupine", "#fafafa", "#212121", "#3264eb", false),
    ("Matte Black", "#121212", "#bebebe", "#e68e0d", true),
    ("Miasma", "#222222", "#c2c2b0", "#78824b", true),
    ("Nord", "#2e3440", "#d8dee9", "#81a1c1", true),
    ("Retro 82", "#05182e", "#f6dcac", "#faa968", true),
    ("Ristretto", "#2c2525", "#e6d9db", "#f38d70", true),
    ("Rose Pine", "#faf4ed", "#575279", "#56949f", false),
    ("Solitude", "#101315", "#cacccc", "#798186", true),
    ("Starship", "#1c1c1e", "#f5f5f7", "#ff9f0a", true),
    ("Tokyo Night", "#1a1b26", "#a9b1d6", "#7aa2f7", true),
    ("Vantablack", "#000000", "#ffffff", "#8d8d8d", true),
    ("White", "#ffffff", "#000000", "#6e6e6e", false),
];

/// ab-nextjs-theme · Material light scheme (`material-theme.json`, 2026).
const AB_LIGHT: &[(&str, &str)] = &[
    ("primary", "#855318"),
    ("onPrimary", "#FFFFFF"),
    ("primaryContainer", "#FFDCBD"),
    ("onPrimaryContainer", "#2C1600"),
    ("secondary", "#6F5D0E"),
    ("onSecondary", "#FFFFFF"),
    ("secondaryContainer", "#FAE287"),
    ("onSecondaryContainer", "#221B00"),
    ("tertiary", "#006A6A"),
    ("onTertiary", "#FFFFFF"),
    ("tertiaryContainer", "#9CF1F0"),
    ("onTertiaryContainer", "#002020"),
    ("error", "#BA1A1A"),
    ("onError", "#FFFFFF"),
    ("errorContainer", "#FFDAD6"),
    ("onErrorContainer", "#410002"),
    ("background", "#FFF8F5"),
    ("onBackground", "#211A14"),
    ("surface", "#FFF8F5"),
    ("onSurface", "#221A15"),
    ("surfaceVariant", "#F2DFD1"),
    ("onSurfaceVariant", "#51453A"),
    ("outline", "#837468"),
    ("outlineVariant", "#D5C3B5"),
    ("surfaceContainerLowest", "#FFFFFF"),
    ("surfaceContainerLow", "#FFF1EA"),
    ("surfaceContainer", "#FCEBE2"),
    ("surfaceContainerHigh", "#F6E5DC"),
    ("surfaceContainerHighest", "#F0DFD7"),
    ("inverseSurface", "#382E29"),
    ("inverseOnSurface", "#FFEDE5"),
];

/// ab-nextjs-theme · Material dark scheme.
const AB_DARK: &[(&str, &str)] = &[
    ("primary", "#FCB975"),
    ("onPrimary", "#4A2800"),
    ("primaryContainer", "#693C00"),
    ("onPrimaryContainer", "#FFDCBD"),
    ("secondary", "#DDC66E"),
    ("onSecondary", "#3A3000"),
    ("secondaryContainer", "#544600"),
    ("onSecondaryContainer", "#FAE287"),
    ("tertiary", "#80D5D4"),
    ("onTertiary", "#003737"),
    ("tertiaryContainer", "#004F50"),
    ("onTertiaryContainer", "#9CF1F0"),
    ("error", "#FFB4AB"),
    ("onError", "#690005"),
    ("errorContainer", "#93000A"),
    ("onErrorContainer", "#FFDAD6"),
    ("background", "#19120C"),
    ("onBackground", "#EFE0D5"),
    ("surface", "#19120D"),
    ("onSurface", "#F0DFD7"),
    ("surfaceVariant", "#51453A"),
    ("onSurfaceVariant", "#D5C3B5"),
    ("outline", "#9D8E81"),
    ("outlineVariant", "#51453A"),
    ("surfaceContainerLowest", "#140D08"),
    ("surfaceContainerLow", "#221A15"),
    ("surfaceContainer", "#261E19"),
    ("surfaceContainerHigh", "#312823"),
    ("surfaceContainerHighest", "#3D332D"),
    ("inverseSurface", "#F0DFD7"),
    ("inverseOnSurface", "#382E29"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_follow_omacalc() {
        let black = Rgb(0, 0, 0);
        let white = Rgb(255, 255, 255);
        assert_eq!(black.mix(white, 0.5), Rgb(128, 128, 128));
        assert_eq!(black.mix(white, 0.0), black);
        assert_eq!(black.mix(white, 1.0), white);
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(Rgb::hex("#f38d70"), Some(Rgb(243, 141, 112)));
        assert_eq!(Rgb::hex("2c2525"), Some(Rgb(44, 37, 37)));
        assert_eq!(Rgb::hex("#fff"), None);
    }

    #[test]
    fn every_builtin_theme_parses() {
        let themes = Theme::all();
        assert_eq!(themes.len(), 23);
        for theme in &themes {
            assert!(Rgb::hex(&theme.background).is_some(), "{}", theme.name);
            assert!(Rgb::hex(&theme.foreground).is_some(), "{}", theme.name);
            assert!(Rgb::hex(&theme.accent).is_some(), "{}", theme.name);
            assert!(theme.css_vars().contains("--op-bg"));
        }
    }

    #[test]
    fn ab_nextjs_light_uses_the_material_palette() {
        let light = Theme::ab_nextjs("light");
        assert_eq!(light.background, "#FFF8F5");
        assert_eq!(light.accent, "#855318");
        assert_eq!(light.foreground, "#211A14");
        assert!(!light.dark);
        let dark = Theme::ab_nextjs("dark");
        assert!(dark.dark);
        assert_eq!(dark.background, "#19120C");
    }

    #[test]
    fn omarchy_dir_reads_colors_and_name() {
        let dir = std::env::temp_dir().join(format!("omapaper-theme-{}", new_id_for_test()));
        let theme_dir = dir.join("theme");
        std::fs::create_dir_all(&theme_dir).unwrap();
        std::fs::write(
            theme_dir.join("colors.toml"),
            "mode = \"dark\"\naccent = \"#f38d70\"\nbackground = \"#2c2525\"\nforeground = \"#e6d9db\"\nred = \"#fd6883\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("theme.name"), "ristretto\n").unwrap();
        let theme = Theme::from_omarchy_dir(&theme_dir).expect("theme");
        assert_eq!(theme.name, "Ristretto");
        assert_eq!(theme.source, "omarchy");
        assert_eq!(theme.palette.red.as_deref(), Some("#fd6883"));
        assert!(theme.dark);
        std::fs::remove_dir_all(&dir).ok();
    }

    fn new_id_for_test() -> String {
        crate::model::new_id()
    }
}
