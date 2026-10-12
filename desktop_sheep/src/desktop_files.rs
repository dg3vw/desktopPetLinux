//! .desktop-Einträge für Autostart und Startmenü, verwaltet per
//! Kommandozeile oder Tray-Menü.

use std::path::PathBuf;

use crate::settings::config_home;

/// Programm-Icon (48×48), auch für das Tray-Icon verwendet.
pub const ICON_PNG: &[u8] = include_bytes!("../packaging/desktop_sheep.png");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopFile {
    /// `$XDG_CONFIG_HOME/autostart/desktop-sheep.desktop`
    Autostart,
    /// `$XDG_DATA_HOME/applications/desktop-sheep.desktop`, dazu das Icon
    /// unter `$XDG_DATA_HOME/icons/`.
    StartMenu,
}

/// `$XDG_DATA_HOME`, sonst `$HOME/.local/share`.
fn data_home() -> Option<PathBuf> {
    match std::env::var_os("XDG_DATA_HOME") {
        Some(p) if !p.is_empty() => Some(PathBuf::from(p)),
        _ => std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")),
    }
}

fn icon_path() -> Option<PathBuf> {
    Some(data_home()?.join("icons/hicolor/48x48/apps/desktop_sheep.png"))
}

fn not_found() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, "HOME nicht gesetzt")
}

impl DesktopFile {
    pub fn path(self) -> Option<PathBuf> {
        Some(match self {
            Self::Autostart => config_home()?.join("autostart/desktop-sheep.desktop"),
            Self::StartMenu => data_home()?.join("applications/desktop-sheep.desktop"),
        })
    }

    pub fn is_installed(self) -> bool {
        self.path().is_some_and(|p| p.exists())
    }

    /// Legt den Eintrag an und liefert seinen Pfad. Gestartet wird ohne
    /// Optionen – Anzahl, Farbe usw. kommen aus den gespeicherten Einstellungen.
    pub fn install(self) -> std::io::Result<PathBuf> {
        // Aus einem AppImage heraus zeigt current_exe() in den temporären Mount
        // (/tmp/.mount_*), der nach dem Beenden verschwindet – dann den Pfad der
        // AppImage-Datei selbst eintragen (setzt die AppImage-Runtime in $APPIMAGE).
        let exe = match std::env::var_os("APPIMAGE") {
            Some(p) => PathBuf::from(p),
            None => std::env::current_exe()?,
        };
        let path = self.path().ok_or_else(not_found)?;
        let extra = match self {
            Self::Autostart => "X-GNOME-Autostart-enabled=true\n".to_string(),
            Self::StartMenu => {
                let icon = icon_path().ok_or_else(not_found)?;
                write_file(&icon, ICON_PNG)?;
                format!(
                    "GenericName=Desktop Pet\n\
                     Icon={}\n\
                     Categories=Utility;Amusement;\n\
                     StartupNotify=false\n",
                    icon.display()
                )
            }
        };
        let content = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Desktop Sheep\n\
             Comment=Ein Schaf läuft an der Bildschirmunterkante\n\
             Exec=\"{}\"\n\
             Terminal=false\n\
             {extra}",
            exe.display()
        );
        write_file(&path, content.as_bytes())?;
        Ok(path)
    }

    /// Entfernt den Eintrag. `Ok(false)`, wenn keiner vorhanden war.
    pub fn remove(self) -> std::io::Result<bool> {
        if self == Self::StartMenu {
            if let Some(icon) = icon_path() {
                remove_file(&icon)?;
            }
        }
        remove_file(&self.path().ok_or_else(not_found)?)
    }

    /// Schaltet den Eintrag um (Tray-Menü).
    pub fn toggle(self) -> std::io::Result<()> {
        if self.is_installed() {
            self.remove().map(|_| ())
        } else {
            self.install().map(|_| ())
        }
    }
}

fn write_file(path: &std::path::Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, content)
}

fn remove_file(path: &std::path::Path) -> std::io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}
