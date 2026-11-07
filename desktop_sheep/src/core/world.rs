use super::surfaces::Surface;

/// Alles, was ein Schaf pro Tick über seine Umgebung wissen muss.
/// Indizes entsprechen `Sheep::monitor_idx` bzw. den Overlays im Backend.
pub struct World<'a> {
    /// Live-Maße jeder Overlay-Zeichenfläche (width, height).
    pub area_dims: &'a [(i32, i32)],
    /// Begehbare Fensterkanten pro Overlay (leer, wenn Fenster-Laufen aus ist).
    pub surfaces: &'a [Vec<Surface>],
    /// Pro Overlay: Liegt es über einem sichtbaren Vollbild-Fenster? Schafe
    /// meiden solche Monitore, solange es einen freien gibt.
    pub fullscreen: &'a [bool],
}

impl World<'_> {
    pub fn dims(&self, monitor_idx: usize) -> (i32, i32) {
        self.area_dims.get(monitor_idx).copied().unwrap_or((0, 0))
    }

    pub fn surfaces(&self, monitor_idx: usize) -> &[Surface] {
        self.surfaces
            .get(monitor_idx)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn is_fullscreen(&self, monitor_idx: usize) -> bool {
        self.fullscreen.get(monitor_idx).copied().unwrap_or(false)
    }

    /// Alle Monitore ohne Vollbild-Fenster.
    pub fn free_monitors(&self) -> Vec<usize> {
        (0..self.area_dims.len())
            .filter(|&i| !self.is_fullscreen(i))
            .collect()
    }

    /// Kennen alle Overlays schon ihre Maße?
    pub fn overlays_ready(&self) -> bool {
        !self.area_dims.is_empty() && self.area_dims.iter().all(|&(_, h)| h > 0)
    }
}
