use super::surfaces::Surface;

/// Alles, was ein Schaf pro Tick über seine Umgebung wissen muss.
/// Indizes entsprechen `Sheep::monitor_idx` bzw. den Overlays im Backend.
pub struct World<'a> {
    /// Live-Maße jeder Overlay-Zeichenfläche (width, height).
    pub area_dims: &'a [(i32, i32)],
    /// Begehbare Fensterkanten pro Overlay (leer, wenn Fenster-Laufen aus ist).
    pub surfaces: &'a [Vec<Surface>],
}

impl World<'_> {
    pub fn dims(&self, monitor_idx: usize) -> (i32, i32) {
        self.area_dims.get(monitor_idx).copied().unwrap_or((0, 0))
    }

    pub fn surfaces(&self, monitor_idx: usize) -> &[Surface] {
        self.surfaces.get(monitor_idx).map(Vec::as_slice).unwrap_or(&[])
    }
}
