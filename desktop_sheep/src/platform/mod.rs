use crate::core::geometry::Rect;
use crate::core::sheep::Sheep;
use crate::core::world::World;

/// Backend-Interface: Linux/X11 heute, Windows/macOS potentiell später.
/// Der Kern (core::) kennt nichts von GTK/X11 und arbeitet nur mit diesen
/// abstrakten Typen.
pub trait Platform {
    /// Sichtbare Monitor-Rechtecke im virtuellen Desktop-Koordinatensystem.
    fn monitor_geometry(&self) -> Vec<Rect>;

    /// Startet Event-Loop des Backends. Blockiert, bis das Overlay geschlossen wird.
    /// Der `tick`-Callback wird periodisch aufgerufen und darf die Sheep-Zustände
    /// mutieren; der Renderer liest sie danach zum Zeichnen.
    /// `tick`-Signatur: (&mut [Sheep], dt_ms, world).
    /// `world.area_dims[i] = (width, height)` der i-ten Overlay-DrawingArea,
    /// in derselben Reihenfolge wie `monitor_geometry()`. Kann kleiner sein
    /// als das theoretische Monitorrechteck (Panels, WM-Struts) – deshalb
    /// authoritativ für Wenden/Boden. `world.surfaces[i]` sind die
    /// begehbaren Fensterkanten auf diesem Overlay (Plan Abschnitt 6).
    fn run<TickFn>(self, sheeps: Vec<Sheep>, tick_ms: u32, tick: TickFn) -> Result<(), String>
    where
        TickFn: FnMut(&mut [Sheep], u32, &World) + 'static;
}

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
mod x11_windows;

#[cfg(target_os = "linux")]
pub use linux::LinuxPlatform as CurrentPlatform;
