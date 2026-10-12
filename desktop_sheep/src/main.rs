mod core;
mod desktop_files;
mod platform;
mod settings;

use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::geometry::ground_segments;
use crate::core::sheep::{avoid_collisions, Entry, Sheep};
use crate::desktop_files::DesktopFile;
use crate::platform::{CurrentPlatform, Platform};
use crate::settings::{ColorChoice, Settings};

#[derive(Debug)]
enum Command {
    Run(Settings),
    Install(DesktopFile),
    Remove(DesktopFile),
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
           {bin} --install-menu           Eintrag im Startmenü anlegen (~/.local/share/applications/)\n  \
           {bin} --remove-menu            Startmenü-Eintrag entfernen\n  \
           {bin} --help                   diese Hilfe\n\
         \n\
         Optionen (überschreiben die im Tray-Menü gespeicherten Einstellungen):\n  \
           --sheep N        Anzahl Schafe (Standard: 1)\n  \
           --color FARBE    default | blue | green | orange | pink | purple | red | yellow\n  \
                            | random (jedes Schaf zufällig)\n  \
           --no-windows     nur am Boden laufen, nicht auf Fensteroberkanten\n  \
           --windows        auch auf Fensteroberkanten laufen (Standard)\n"
    );
}

fn parse_args() -> Command {
    let mut settings = Settings::load();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" => return Command::Help,
            "--install-autostart" => return Command::Install(DesktopFile::Autostart),
            "--remove-autostart" => return Command::Remove(DesktopFile::Autostart),
            "--install-menu" => return Command::Install(DesktopFile::StartMenu),
            "--remove-menu" => return Command::Remove(DesktopFile::StartMenu),
            "--no-windows" => settings.walk_on_windows = false,
            "--windows" => settings.walk_on_windows = true,
            "--sheep" => {
                if let Some(v) = it.next() {
                    settings.n_sheep = v.parse().unwrap_or(1).max(1);
                }
            }
            "--color" => {
                let Some(v) = it.next() else {
                    eprintln!("--color: Farbe fehlt");
                    return Command::Help;
                };
                match ColorChoice::from_name(&v) {
                    Some(c) => settings.color = c,
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
    Command::Run(settings)
}

fn main() {
    match parse_args() {
        Command::Help => print_help(),
        Command::Install(f) => match f.install() {
            Ok(path) => println!("Eingerichtet: {}", path.display()),
            Err(e) => {
                eprintln!("Einrichten fehlgeschlagen: {e}");
                std::process::exit(1);
            }
        },
        Command::Remove(f) => match f.remove() {
            Ok(true) => println!("Eintrag entfernt."),
            Ok(false) => println!("Kein Eintrag vorhanden."),
            Err(e) => {
                eprintln!("Entfernen fehlgeschlagen: {e}");
                std::process::exit(1);
            }
        },
        Command::Run(settings) => run_overlay(settings),
    }
}

fn run_overlay(settings: Settings) {
    let platform = CurrentPlatform::new(settings);
    let monitors = platform.monitor_geometry();
    if monitors.is_empty() {
        eprintln!("Keine Monitore gefunden – bricht ab.");
        std::process::exit(1);
    }
    let segments = ground_segments(&monitors);

    // Schaf Nr. i: rundum auf die Monitore verteilt, jedes mit eigenem Seed.
    // Der Zähler sorgt dafür, dass ein im Menü neu hinzugefügtes Schaf nicht
    // denselben Seed wie ein zuvor entferntes bekommt.
    let n_monitors = monitors.len();
    let base_seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xDEAD_BEEF_C0FFEE);
    let mut spawned: u64 = 0;
    let mut spawn = move |i: usize| {
        let monitor_idx = i % n_monitors;
        // Startpositionen ~150 px voneinander, damit sie sich nicht sofort überlappen.
        let x = 40.0 + ((i / n_monitors) as f32) * 150.0;
        let seed = base_seed.wrapping_add(spawned.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        spawned += 1;
        Sheep::new_at(monitor_idx, x, seed)
    };

    // Zum Ausprobieren der seltenen Auftritte: DESKTOP_SHEEP_ENTRY=bath|visit|sky
    // lässt jedes Schaf beim Start so ins Bild kommen.
    let debug_entry = std::env::var("DESKTOP_SHEEP_ENTRY")
        .ok()
        .and_then(|v| Entry::from_name(&v));
    let sheeps: Vec<Sheep> = (0..settings.n_sheep)
        .map(|i| {
            let mut s = spawn(i);
            if let Some(e) = debug_entry {
                s.set_entry(e);
            }
            s
        })
        .collect();

    let seg_for_tick = segments.clone();
    if let Err(e) = platform.run(sheeps, spawn, 16, move |all, dt, world| {
        for s in all.iter_mut() {
            s.tick(dt, &seg_for_tick, world);
        }
        avoid_collisions(all, world);
    }) {
        eprintln!("Backend-Fehler: {e}");
        std::process::exit(1);
    }
}
