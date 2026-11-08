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
- Stört nicht bei Spielen und Videos: Monitore mit sichtbarem Vollbild-Fenster
  meiden die Schafe. Wer dort ist, fällt auf einem freien Monitor neu ins Bild,
  und am Rand zum Vollbild-Monitor kehren sie um. Ist kein Monitor frei, laufen
  sie unsichtbar und klickdurchlässig dahinter weiter; nur der Eieruhr-Alarm
  wird auch dann angezeigt
- Klettert an der äußeren Bildschirmwand hoch, läuft kopfüber an der
  Bildschirmoberkante entlang und lässt sich gelegentlich fallen – so
  landet es von selbst wieder auf Fenstern; sonst klettert es an der
  gegenüberliegenden Wand wieder herunter
- Fällt beim Start von oben ins Bild und landet dabei ggf. auf einem Fenster;
  läuft gelegentlich seitlich aus dem Bild und kommt wieder – meist von oben,
  manchmal als brennender Meteor, der in einer Badewanne landet, oder es
  trifft in der Bildmitte ein schwarzes Schaf und errötet
- Ab und zu schwebt ein UFO heran, während das Schaf grast, und beamt es
  im Lichtkegel nach oben; das UFO fliegt davon, und kurz darauf kommt das
  Schaf wieder ins Bild. Wer das Schaf rechtzeitig aus dem Strahl zieht,
  rettet es – dann fliegt das UFO ohne Beute ab
- Frisst gelegentlich eine Blume Bissen für Bissen ab: Maul auf, zubeißen,
  kauen – bis zuletzt auch der Stängel verputzt ist
- Rollt nach einem schnellen Wurf über den Boden und bleibt kurz benommen
  sitzen; rollt es gegen eine Wand, tut das weh, über eine Fensterkante
  kullert es hinunter
- Grasen, Gähnen/Schlafen, Sitzen/Dösen, von vorn sitzend schlafen –
  Mausbewegung weckt schlafende Schafe
- Schaf mit der Maus packen und wegziehen, auch über Monitorgrenzen; beim
  Loslassen fliegt es mit dem Schwung der Mausbewegung im Bogen weiter,
  prallt vom Bildschirmrand ab, fällt mit Schwerkraft und federt nach – auf
  den Boden oder auf ein Fenster darunter
- Mehrere Schafe gleichzeitig (`--sheep N`); sie laufen nicht durcheinander
  hindurch, sondern springen übereinander hinweg (weit genug, auch wenn
  das vordere davonläuft) oder drehen um
- Farbvarianten (`--color`), auch zufällig je Schaf (`--color random`)
- Eieruhr im Tray-Menü (1 bis 60 Minuten): Die Schafe laufen normal weiter;
  zeigt die Maus auf ein Schaf, nennt es in einer Sprechblase die Restzeit.
  Ist die Zeit um, hüpfen alle Schafe auf der Stelle, eines zeigt
  „Zeit ist um!“, und einmal erklingt der Systemklang
  `alarm-clock-elapsed` (über `paplay`, `pw-play` oder
  `canberra-gtk-play`, falls vorhanden). Ein Klick auf ein Schaf oder die
  Blase – oder „Alarm beenden“ im Tray – beendet den Alarm
- Tray-Menü: Timer, Anzahl Schafe, Farbe, „Auf Fenstern laufen“, „Beim
  Anmelden starten“ und „Im Startmenü anzeigen“ – wirkt sofort und wird
  gespeichert (außer dem Timer); dazu „Beenden“
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
`DESKTOP_SHEEP_ENTRY=visit` (schwarzes Schaf) bzw.
`DESKTOP_SHEEP_ENTRY=ufo` (UFO-Entführung) bzw.
`DESKTOP_SHEEP_ENTRY=flower` (Blume fressen).
`DESKTOP_SHEEP_TIMER=<sekunden>` startet gleich beim Start einen Timer.

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
  timer.rs           Eieruhr: Countdown-Zustand, Zeitformate
  desktop_files.rs   .desktop-Einträge für Autostart und Startmenü
  core/              plattformunabhängig (kein GTK/X11)
    geometry.rs      Monitor-Rechtecke → Boden-Segmente
    surfaces.rs      Fensterrechtecke → begehbare, unverdeckte Oberkanten
    world.rs         Tick-Kontext (Overlay-Maße + Kanten)
    anim.rs          Animations-Tabelle (aus animations.xml abgeleitet), RNG
    bubble.rs        Lage einer Sprechblase über/unter dem Sprite
    fling.rs         Wurfgeschwindigkeit beim Loslassen aus der Mausbewegung
    sheep.rs         Zustandsmaschine, Bewegung, Fall-Physik, Fenster-Halt
  platform/
    mod.rs           Platform-Trait
    linux.rs         GTK4/X11-Backend: ein Overlay pro Monitor, Input-Region,
                     Drag, WM-Hints, Tray-Menü (ksni), Fenster-Poller,
                     Sprechblasen, Timer-Alarm samt Ton
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
