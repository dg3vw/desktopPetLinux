# desktop_sheep – eSheep für Linux

Rust/GTK4-Portierung des klassischen eSheep: Ein (oder mehrere) Schafe laufen
an der Bildschirmunterkante und auf den Oberkanten offener Fenster entlang –
über alle Monitore hinweg, in einem transparenten Overlay, das nur über dem
Schaf selbst Mausklicks annimmt.

Zielumgebung ist **LMDE 7 / Cinnamon unter X11**. Wayland wird nicht
unterstützt.

## Features

- Laufen, Sprinten, Umdrehen, gelegentlich gegen die Wand rennen
- Laufen auf Fensteroberkanten: das Schaf wandert mit, wenn das Fenster
  verschoben wird, fällt herunter, wenn es geschlossen, minimiert oder
  verdeckt wird, und springt an der Kante herunter oder rutscht ab
- Fällt beim Start von oben ins Bild und landet dabei ggf. auf einem Fenster;
  läuft gelegentlich seitlich aus dem Bild und kommt von oben wieder
- Grasen, Gähnen/Schlafen, Sitzen/Dösen – Mausbewegung weckt schlafende Schafe
- Schaf mit der Maus packen und wegziehen, auch über Monitorgrenzen; beim
  Loslassen fällt es mit Schwerkraft und federt nach – auf den Boden oder
  auf ein Fenster darunter
- Mehrere Schafe gleichzeitig (`--sheep N`)
- Farbvarianten (`--color`)
- Tray-Icon mit „Beenden“
- Autostart-Eintrag per Kommandozeile

## Bauen

Abhängigkeiten (Debian/LMDE):

```sh
sudo apt install build-essential pkg-config libgtk-4-dev libx11-dev libdbus-1-dev
```

Rust ≥ 1.80 über [rustup](https://rustup.rs). Dann:

```sh
cargo build --release
./target/release/desktop_sheep
```

## Nutzung

```
desktop_sheep [OPTIONEN]              starten
desktop_sheep --install-autostart     ~/.config/autostart/desktop-sheep.desktop anlegen
desktop_sheep --remove-autostart      Autostart-Eintrag entfernen
desktop_sheep --help

Optionen:
  --sheep N        Anzahl Schafe (Standard: 1)
  --color FARBE    default | blue | green | orange | pink | purple | red | yellow
  --no-windows     nur am Boden laufen, nicht auf Fensteroberkanten
```

Beenden über das Tray-Icon oder `Ctrl+C` im Terminal.

Mit `DESKTOP_SHEEP_DEBUG=1` gibt das Programm bei jeder Änderung die
erkannten Fenster und begehbaren Kanten auf stderr aus.

## Tests

```sh
cargo test
```

Deckt die Kantenberechnung (Verdeckung, Monitor-Clipping) und die
Fall-/Lande-Logik des Schafs ab.

## AppImage

```sh
./packaging/make_appimage.sh
```

Lädt beim ersten Lauf `linuxdeploy` und das GTK-Plugin nach
`packaging/.cache/` und erzeugt `packaging/build/Desktop_Sheep-x86_64.AppImage`.
Die glibc wird nicht mitgebündelt – das AppImage läuft auf Systemen mit gleich
alter oder neuerer glibc als der Build-Rechner.

## Aufbau

```
src/
  main.rs            CLI, Schafe erzeugen, Tick-Callback
  core/              plattformunabhängig (kein GTK/X11)
    geometry.rs      Monitor-Rechtecke → Boden-Segmente
    surfaces.rs      Fensterrechtecke → begehbare, unverdeckte Oberkanten
    world.rs         Tick-Kontext (Overlay-Maße + Kanten)
    anim.rs          Animations-Tabelle (aus animations.xml abgeleitet), RNG
    sheep.rs         Zustandsmaschine, Bewegung, Fall-Physik, Fenster-Halt
  platform/
    mod.rs           Platform-Trait
    linux.rs         GTK4/X11-Backend: ein Overlay pro Monitor, Input-Region,
                     Drag, WM-Hints, Tray (ksni), Fenster-Poller
    x11_windows.rs   Fensterliste per EWMH (_NET_CLIENT_LIST_STACKING,
                     Frame-/CSD-Extents, Filter für Desktop/Dock/minimiert)
assets/              Sprite-Sheets (40×40-Kacheln, 16 Spalten)
packaging/           AppImage-Skript, .desktop, Icon
```

## Sprites / Lizenz

Die Sprite-Sheets stammen aus dem Upstream-Projekt
[Adrianotiger/desktopPet](https://github.com/Adrianotiger/desktopPet)
(`Pets/esheep64` bzw. die gSheep-Farbvarianten; laut dortigem README
„sprites from LiL_Stenly“). Vor einer Weiterverbreitung die Lizenzlage
im Upstream-Repo prüfen.
