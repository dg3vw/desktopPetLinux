mod core;
mod platform;

use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::anim::SpriteColor;
use crate::core::geometry::ground_segments;
use crate::core::sheep::Sheep;
use crate::platform::{CurrentPlatform, Platform};

#[derive(Debug)]
enum Command {
    Run { n_sheep: usize, color: SpriteColor, windows: bool },
    InstallAutostart,
    RemoveAutostart,
    Help,
}

fn print_help() {
    let bin = std::env::args().next().unwrap_or_else(|| "desktop_sheep".into());
    println!(
        "desktop_sheep – Overlay-Schafe an der Bildschirmunterkante.\n\
         \n\
         Nutzung:\n  \
           {bin} [OPTIONEN]               starten\n  \
           {bin} --install-autostart      .desktop-Datei in ~/.config/autostart/ anlegen\n  \
           {bin} --remove-autostart       .desktop-Datei entfernen\n  \
           {bin} --help                   diese Hilfe\n\
         \n\
         Optionen:\n  \
           --sheep N        Anzahl Schafe (Standard: 1)\n  \
           --color FARBE    default | blue | green | orange | pink | purple | red | yellow\n  \
           --no-windows     nur am Boden laufen, nicht auf Fensteroberkanten\n"
    );
}

fn parse_args() -> Command {
    let mut n_sheep: usize = 1;
    let mut color: SpriteColor = SpriteColor::Default;
    let mut windows = true;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" => return Command::Help,
            "--install-autostart" => return Command::InstallAutostart,
            "--remove-autostart" => return Command::RemoveAutostart,
            "--no-windows" => windows = false,
            "--sheep" => {
                if let Some(v) = it.next() {
                    n_sheep = v.parse().unwrap_or(1).max(1);
                }
            }
            "--color" => {
                let Some(v) = it.next() else {
                    eprintln!("--color: Farbe fehlt");
                    return Command::Help;
                };
                match SpriteColor::from_name(&v) {
                    Some(c) => color = c,
                    None => {
                        eprintln!("Unbekannte Farbe: {v}");
                        return Command::Help;
                    }
                }
            }
            other => {
                eprintln!("Unbekanntes Argument: {other}");
                return Command::Help;
            }
        }
    }
    Command::Run { n_sheep, color, windows }
}

fn autostart_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    let mut p = std::path::PathBuf::from(home);
    p.push(".config/autostart/desktop-sheep.desktop");
    Some(p)
}

fn install_autostart() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let path = autostart_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "HOME nicht gesetzt")
    })?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Desktop Sheep\n\
         Comment=Ein Schaf läuft an der Bildschirmunterkante\n\
         Exec={} --sheep 1\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        exe.display()
    );
    std::fs::write(&path, content)?;
    println!("Autostart eingerichtet: {}", path.display());
    Ok(())
}

fn remove_autostart() -> std::io::Result<()> {
    let path = autostart_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "HOME nicht gesetzt")
    })?;
    match std::fs::remove_file(&path) {
        Ok(_) => {
            println!("Autostart entfernt: {}", path.display());
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("Kein Autostart-Eintrag vorhanden ({}).", path.display());
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn main() {
    match parse_args() {
        Command::Help => {
            print_help();
            return;
        }
        Command::InstallAutostart => {
            if let Err(e) = install_autostart() {
                eprintln!("Autostart-Installation fehlgeschlagen: {e}");
                std::process::exit(1);
            }
            return;
        }
        Command::RemoveAutostart => {
            if let Err(e) = remove_autostart() {
                eprintln!("Autostart-Entfernen fehlgeschlagen: {e}");
                std::process::exit(1);
            }
            return;
        }
        Command::Run { n_sheep, color, windows } => run_overlay(n_sheep, color, windows),
    }
}

fn run_overlay(n_sheep: usize, color: SpriteColor, windows: bool) {
    let platform = CurrentPlatform::new(color, windows);
    let monitors = platform.monitor_geometry();
    if monitors.is_empty() {
        eprintln!("Keine Monitore gefunden – bricht ab.");
        std::process::exit(1);
    }
    let segments = ground_segments(&monitors);

    // N Schafe rundum auf die Monitore verteilen, jedes mit eigenem Seed.
    let base_seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xDEAD_BEEF_C0FFEE);
    let mut sheeps = Vec::with_capacity(n_sheep);
    for i in 0..n_sheep {
        let monitor_idx = i % monitors.len();
        // Startpositionen ~150 px voneinander, damit sie sich nicht sofort überlappen.
        let x = 40.0 + ((i / monitors.len()) as f32) * 150.0;
        let seed = base_seed
            .wrapping_add((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        sheeps.push(Sheep::new_at(monitor_idx, x, seed));
    }

    let seg_for_tick = segments.clone();
    if let Err(e) = platform.run(sheeps, 16, move |all, dt, world| {
        for s in all.iter_mut() {
            s.tick(dt, &seg_for_tick, world);
        }
    }) {
        eprintln!("Backend-Fehler: {e}");
        std::process::exit(1);
    }
}
