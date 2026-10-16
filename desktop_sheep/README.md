# desktop_sheep – eSheep für Linux

Rust/GTK4-Portierung des klassischen eSheep: Ein (oder mehrere) Schafe laufen
an der Bildschirmunterkante und auf den Oberkanten offener Fenster entlang –
über alle Monitore hinweg, in einem transparenten Overlay, das nur über dem
Schaf selbst Mausklicks annimmt.

Zielumgebung ist **LMDE 7 / Cinnamon unter X11**. Wayland wird nicht
unterstützt.

## Features

- Laufen, Sprinten, Bocksprünge, Umdrehen, gelegentlich gegen die Wand rennen
- Laufen auf Fensteroberkanten: zieht man das Fenster unter dem Schaf weg
  (oder schließt, minimiert, verdeckt es), hängt es kurz verdutzt in der
  Luft und fällt dann auf das nächste Fenster oder den Boden; an der
  Fensterkante springt es herunter oder rutscht ab
- Klettert an der äußeren Bildschirmwand hoch, läuft kopfüber an der
  Bildschirmoberkante entlang und lässt sich gelegentlich fallen – so
  landet es von selbst wieder auf Fenstern; sonst klettert es an der
  gegenüberliegenden Wand wieder herunter
- Fällt beim Start von oben ins Bild und landet dabei ggf. auf einem Fenster;
  läuft gelegentlich seitlich aus dem Bild und kommt wieder – meist von oben,
  manchmal als brennender Meteor, der in einer Badewanne landet, oder es
  trifft in der Bildmitte ein schwarzes Schaf und errötet
- Grasen, Gähnen/Schlafen, Sitzen/Dösen, von vorn sitzend schlafen –
  Mausbewegung weckt schlafende Schafe
- Schaf mit der Maus packen und wegziehen, auch über Monitorgrenzen; beim
  Loslassen fällt es mit Schwerkraft und federt nach – auf den Boden oder
  auf ein Fenster darunter
- Mehrere Schafe gleichzeitig (`--sheep N`); sie laufen nicht durcheinander
  hindurch, sondern springen übereinander hinweg oder drehen um
- Farbvarianten (`--color`), auch zufällig je Schaf (`--color random`)
- Tray-Menü: Anzahl Schafe, Farbe, „Auf Fenstern laufen“, „Beim
  Anmelden starten“ und „Im Startmenü anzeigen“ – wirkt sofort und wird
  gespeichert; dazu „Beenden“
- Autostart- und Startmenü-Eintrag per Tray-Menü oder Kommandozeile
- Läuft nur einmal: ein weiterer Start (z. B. aus dem Startmenü) beendet
  sich sofort, wenn schon Schafe unterwegs sind

## Bauen

Abhängigkeiten (Debian/LMDE):

```sh
sudo apt install build-essential pkg-config libgtk-4-dev libx11-dev libdbus-1-dev
```

Rust ≥ 1.85 über [rustup](https://rustup.rs). Dann:

```sh
cargo build --release
./target/release/desktop_sheep
```

## Nutzung

```
desktop_sheep [OPTIONEN]              starten
desktop_sheep --install-autostart     ~/.config/autostart/desktop-sheep.desktop anlegen
desktop_sheep --remove-autostart      Autostart-Eintrag entfernen
desktop_sheep --install-menu          Startmenü-Eintrag in ~/.local/share/applications/ anlegen
desktop_sheep --remove-menu           Startmenü-Eintrag entfernen
desktop_sheep --help

Optionen (überschreiben die im Tray-Menü gespeicherten Einstellungen):
  --sheep N        Anzahl Schafe (Standard: 1)
  --color FARBE    default | blue | green | orange | pink | purple | red | yellow
                   | random (jedes Schaf zufällig)
  --no-windows     nur am Boden laufen, nicht auf Fensteroberkanten
  --windows        auch auf Fensteroberkanten laufen (Standard)
```

Dieselben Einstellungen lassen sich zur Laufzeit im Menü des Tray-Icons
ändern. Sie werden in `~/.config/desktop_sheep/settings.conf` gespeichert
und beim nächsten Start (auch per Autostart) übernommen. Optionen auf der
Kommandozeile gelten nur für diesen Lauf – gespeichert wird erst, wenn im
Menü etwas geändert wird.

Beenden über das Tray-Icon oder `Ctrl+C` im Terminal.

Mit `DESKTOP_SHEEP_DEBUG=1` gibt das Programm bei jeder Änderung die
erkannten Fenster und begehbaren Kanten auf stderr aus.

Die seltenen Auftritte lassen sich direkt beim Start erzwingen:
`DESKTOP_SHEEP_ENTRY=bath` (Meteor in die Badewanne) bzw.
`DESKTOP_SHEEP_ENTRY=visit` (schwarzes Schaf).

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
  settings.rs        gespeicherte Einstellungen (settings.conf)
  desktop_files.rs   .desktop-Einträge für Autostart und Startmenü
  core/              plattformunabhängig (kein GTK/X11)
    geometry.rs      Monitor-Rechtecke → Boden-Segmente
    surfaces.rs      Fensterrechtecke → begehbare, unverdeckte Oberkanten
    world.rs         Tick-Kontext (Overlay-Maße + Kanten)
    anim.rs          Animations-Tabelle (aus animations.xml abgeleitet), RNG
    sheep.rs         Zustandsmaschine, Bewegung, Fall-Physik, Fenster-Halt
  platform/
    mod.rs           Platform-Trait
    linux.rs         GTK4/X11-Backend: ein Overlay pro Monitor, Input-Region,
                     Drag, WM-Hints, Tray-Menü (ksni), Fenster-Poller
    x11_windows.rs   Fensterliste per EWMH (_NET_CLIENT_LIST_STACKING,
                     Frame-/CSD-Extents, Filter für Desktop/Dock/minimiert)
assets/              Sprite-Sheets (40×40-Kacheln, 16 Spalten)
packaging/           AppImage-Skript, .desktop, Icon
```

## Sprites / Lizenz

Wie das Upstream-Projekt
[Adrianotiger/desktopPet](https://github.com/Adrianotiger/desktopPet) steht
dieses Repo unter der MIT-Lizenz, siehe [`LICENSE.md`](../LICENSE.md).

Die Sprite-Sheets stammen aus dem Upstream-Projekt (`Pets/esheep64` bzw.
die gSheep-Farbvarianten; laut dortigem README „sprites from LiL_Stenly“)
und werden dort unter dieser Lizenz verbreitet.
