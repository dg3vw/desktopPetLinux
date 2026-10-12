//! Benutzereinstellungen, die per Kommandozeile oder Tray-Menü gesetzt
//! werden. Gespeichert als einfache `schlüssel=wert`-Datei in
//! `$XDG_CONFIG_HOME/desktop_sheep/settings.conf` (Standard: `~/.config/…`).

use std::path::PathBuf;

use crate::core::anim::SpriteColor;
use crate::core::sheep::Sheep;

/// Farbe der Schafe: alle gleich oder jedes zufällig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChoice {
    Single(SpriteColor),
    Random,
}

impl ColorChoice {
    /// Wie bei `--color`: ein Farbname oder `random`.
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "random" | "zufall" => Some(Self::Random),
            other => SpriteColor::from_name(other).map(Self::Single),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Single(c) => c.name(),
            Self::Random => "random",
        }
    }

    /// Färbt ein Schaf entsprechend ein.
    pub fn apply_to(self, sheep: &mut Sheep) {
        match self {
            Self::Single(c) => sheep.color = c,
            Self::Random => sheep.randomize_color(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub n_sheep: usize,
    pub color: ColorChoice,
    /// Fensteroberkanten als Laufflächen anbieten.
    pub walk_on_windows: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { n_sheep: 1, color: ColorChoice::Single(SpriteColor::Default), walk_on_windows: true }
    }
}

/// `$XDG_CONFIG_HOME`, sonst `$HOME/.config`.
pub fn config_home() -> Option<PathBuf> {
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(p) if !p.is_empty() => Some(PathBuf::from(p)),
        _ => std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")),
    }
}

fn settings_path() -> Option<PathBuf> {
    Some(config_home()?.join("desktop_sheep/settings.conf"))
}

impl Settings {
    /// Gespeicherte Einstellungen; fehlt die Datei, die Standardwerte.
    pub fn load() -> Self {
        settings_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| Self::parse(&s))
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = settings_path().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "HOME nicht gesetzt")
        })?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.format())
    }

    /// Unbekannte Schlüssel und ungültige Werte werden ignoriert.
    fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            match key.trim() {
                "sheep" => {
                    if let Ok(n) = value.parse::<usize>() {
                        s.n_sheep = n.max(1);
                    }
                }
                "color" => {
                    if let Some(c) = ColorChoice::from_name(value) {
                        s.color = c;
                    }
                }
                "windows" => {
                    if let Ok(b) = value.parse() {
                        s.walk_on_windows = b;
                    }
                }
                _ => {}
            }
        }
        s
    }

    fn format(&self) -> String {
        format!(
            "sheep={}\ncolor={}\nwindows={}\n",
            self.n_sheep,
            self.color.name(),
            self.walk_on_windows
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = Settings {
            n_sheep: 4,
            color: ColorChoice::Single(SpriteColor::Purple),
            walk_on_windows: false,
        };
        assert_eq!(Settings::parse(&s.format()), s);
        let r = Settings { color: ColorChoice::Random, ..s };
        assert_eq!(Settings::parse(&r.format()), r);
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let s = Settings::parse("sheep=0\ncolor=mauve\nwindows=vielleicht\n# kommentar\n");
        assert_eq!(s, Settings { n_sheep: 1, ..Settings::default() });
    }

    #[test]
    fn random_colors_differ_between_sheep() {
        let colors: std::collections::HashSet<_> = (0..20)
            .map(|seed| {
                let mut s = Sheep::new_at(0, 0.0, seed);
                ColorChoice::Random.apply_to(&mut s);
                s.color
            })
            .collect();
        assert!(colors.len() > 3, "{colors:?}");
    }

    #[test]
    fn color_names_roundtrip() {
        for c in SpriteColor::ALL {
            assert_eq!(SpriteColor::from_name(c.name()), Some(c));
        }
    }
}
