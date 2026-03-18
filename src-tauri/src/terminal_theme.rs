use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalTheme {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub selection_background: String,
    pub selection_foreground: Option<String>,
    pub cursor_accent: Option<String>,
    pub black: Option<String>,
    pub red: Option<String>,
    pub green: Option<String>,
    pub yellow: Option<String>,
    pub blue: Option<String>,
    pub magenta: Option<String>,
    pub cyan: Option<String>,
    pub white: Option<String>,
    pub bright_black: Option<String>,
    pub bright_red: Option<String>,
    pub bright_green: Option<String>,
    pub bright_yellow: Option<String>,
    pub bright_blue: Option<String>,
    pub bright_magenta: Option<String>,
    pub bright_cyan: Option<String>,
    pub bright_white: Option<String>,
}

pub fn default_terminal_theme() -> TerminalTheme {
    TerminalTheme {
        background: "#000000".to_string(),
        foreground: "#e0e0e0".to_string(),
        cursor: "#ffffff".to_string(),
        selection_background: "#2e2e2e".to_string(),
        selection_foreground: None,
        cursor_accent: None,
        black: None,
        red: None,
        green: None,
        yellow: None,
        blue: None,
        magenta: None,
        cyan: None,
        white: None,
        bright_black: None,
        bright_red: None,
        bright_green: None,
        bright_yellow: None,
        bright_blue: None,
        bright_magenta: None,
        bright_cyan: None,
        bright_white: None,
    }
}

pub fn load_terminal_theme(base_dir: &Path) -> io::Result<TerminalTheme> {
    let path = base_dir.join("current-theme.conf");
    tracing::debug!(?path, "loading terminal theme");
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => {
            tracing::debug!(bytes = contents.len(), "read theme file");
            contents
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            tracing::debug!("theme file not found, using default theme");
            return Ok(default_terminal_theme());
        }
        Err(error) => {
            tracing::error!(?path, %error, "failed to read theme file");
            return Err(error);
        }
    };

    parse_terminal_theme(&contents).or_else(|e| {
        tracing::warn!(%e, "invalid theme data, falling back to default theme");
        Ok(default_terminal_theme())
    })
}

fn parse_terminal_theme(contents: &str) -> io::Result<TerminalTheme> {
    let mut theme = default_terminal_theme();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else {
            continue;
        };
        let Some(value) = parts.next() else {
            tracing::warn!(key, "theme line missing color value");
            continue;
        };
        let color = parse_color(value)?;

        match key {
            "background" => theme.background = color,
            "foreground" => theme.foreground = color,
            "cursor" => theme.cursor = color,
            "selection_background" => theme.selection_background = color,
            "selection_foreground" => theme.selection_foreground = Some(color),
            "cursor_text_color" => theme.cursor_accent = Some(color),
            "color0" => theme.black = Some(color),
            "color1" => theme.red = Some(color),
            "color2" => theme.green = Some(color),
            "color3" => theme.yellow = Some(color),
            "color4" => theme.blue = Some(color),
            "color5" => theme.magenta = Some(color),
            "color6" => theme.cyan = Some(color),
            "color7" => theme.white = Some(color),
            "color8" => theme.bright_black = Some(color),
            "color9" => theme.bright_red = Some(color),
            "color10" => theme.bright_green = Some(color),
            "color11" => theme.bright_yellow = Some(color),
            "color12" => theme.bright_blue = Some(color),
            "color13" => theme.bright_magenta = Some(color),
            "color14" => theme.bright_cyan = Some(color),
            "color15" => theme.bright_white = Some(color),
            _ => {
                tracing::warn!(key, "unrecognized theme key");
            }
        }
    }

    tracing::debug!("terminal theme parsed");
    Ok(theme)
}

fn parse_color(value: &str) -> io::Result<String> {
    let valid_len = value.len() == 7 || value.len() == 9;
    let valid_hex = value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit());
    if valid_len && valid_hex {
        Ok(value.to_ascii_lowercase())
    } else {
        tracing::error!(value, "invalid color value in theme");
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid color value: {value}"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_terminal_theme_returns_default_when_theme_file_is_missing() {
        let tmp = TempDir::new().expect("temp dir");

        let theme = load_terminal_theme(tmp.path()).expect("theme should load");

        assert_eq!(theme, default_terminal_theme());
    }

    #[test]
    fn test_load_terminal_theme_maps_kitty_keys_to_xterm_theme() {
        let tmp = TempDir::new().expect("temp dir");
        fs::write(
            tmp.path().join("current-theme.conf"),
            "\
background #111111
foreground #eeeeee
cursor #ffcc00
selection_background #333333
selection_foreground #fafafa
cursor_text_color #101010
color0 #010101
color8 #080808
color15 #f5f5f5
",
        )
        .expect("write theme");

        let theme = load_terminal_theme(tmp.path()).expect("theme should load");

        assert_eq!(theme.background, "#111111");
        assert_eq!(theme.foreground, "#eeeeee");
        assert_eq!(theme.cursor, "#ffcc00");
        assert_eq!(theme.selection_background, "#333333");
        assert_eq!(theme.selection_foreground.as_deref(), Some("#fafafa"));
        assert_eq!(theme.cursor_accent.as_deref(), Some("#101010"));
        assert_eq!(theme.black.as_deref(), Some("#010101"));
        assert_eq!(theme.bright_black.as_deref(), Some("#080808"));
        assert_eq!(theme.bright_white.as_deref(), Some("#f5f5f5"));
    }

    #[test]
    fn test_load_terminal_theme_returns_default_when_theme_contains_invalid_color() {
        let tmp = TempDir::new().expect("temp dir");
        fs::write(
            tmp.path().join("current-theme.conf"),
            "\
background not-a-color
foreground #eeeeee
",
        )
        .expect("write theme");

        let theme = load_terminal_theme(tmp.path()).expect("theme should load");

        assert_eq!(theme, default_terminal_theme());
    }
}
