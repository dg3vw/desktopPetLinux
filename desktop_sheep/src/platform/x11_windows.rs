//! Fensterliste per EWMH.
//!
//! Liest `_NET_CLIENT_LIST_STACKING` und liefert die sichtbaren fremden
//! Top-Level-Fenster des aktuellen Arbeitsbereichs in Stapelreihenfolge
//! (unten → oben) mit ihrem sichtbaren Rechteck in Root-Koordinaten.
//!
//! Alle Funktionen hier müssen innerhalb eines GDK-Error-Traps laufen:
//! Fenster können zwischen dem Lesen der Liste und der Geometrie-Abfrage
//! verschwinden, und ein unbehandeltes `BadWindow` würde den Prozess beenden.

use std::ffi::CString;
use std::os::raw::{c_int, c_uchar, c_ulong};

use x11::xlib;

use crate::core::geometry::Rect;
use crate::core::surfaces::WindowRect;

pub struct Atoms {
    client_list_stacking: xlib::Atom,
    current_desktop: xlib::Atom,
    wm_desktop: xlib::Atom,
    wm_pid: xlib::Atom,
    wm_window_type: xlib::Atom,
    type_desktop: xlib::Atom,
    type_dock: xlib::Atom,
    wm_state: xlib::Atom,
    state_hidden: xlib::Atom,
    frame_extents: xlib::Atom,
    gtk_frame_extents: xlib::Atom,
}

impl Atoms {
    pub unsafe fn new(d: *mut xlib::Display) -> Self {
        let intern = |name: &str| {
            let c = CString::new(name).unwrap();
            xlib::XInternAtom(d, c.as_ptr(), 0)
        };
        Self {
            client_list_stacking: intern("_NET_CLIENT_LIST_STACKING"),
            current_desktop: intern("_NET_CURRENT_DESKTOP"),
            wm_desktop: intern("_NET_WM_DESKTOP"),
            wm_pid: intern("_NET_WM_PID"),
            wm_window_type: intern("_NET_WM_WINDOW_TYPE"),
            type_desktop: intern("_NET_WM_WINDOW_TYPE_DESKTOP"),
            type_dock: intern("_NET_WM_WINDOW_TYPE_DOCK"),
            wm_state: intern("_NET_WM_STATE"),
            state_hidden: intern("_NET_WM_STATE_HIDDEN"),
            frame_extents: intern("_NET_FRAME_EXTENTS"),
            gtk_frame_extents: intern("_GTK_FRAME_EXTENTS"),
        }
    }
}

/// `_NET_WM_DESKTOP`-Wert für "auf allen Arbeitsbereichen".
const ALL_DESKTOPS: u64 = 0xFFFF_FFFF;

/// Liest eine Property im 32-Bit-Format (CARDINAL, WINDOW, ATOM, …).
/// Xlib liefert Format-32-Daten als `long`-Array, auch auf 64-Bit-Systemen.
unsafe fn get_u32_list(
    d: *mut xlib::Display,
    w: xlib::Window,
    prop: xlib::Atom,
) -> Option<Vec<u64>> {
    let mut actual_type: xlib::Atom = 0;
    let mut actual_format: c_int = 0;
    let mut nitems: c_ulong = 0;
    let mut bytes_after: c_ulong = 0;
    let mut data: *mut c_uchar = std::ptr::null_mut();
    let status = xlib::XGetWindowProperty(
        d,
        w,
        prop,
        0,
        4096,
        0,
        0, // AnyPropertyType
        &mut actual_type,
        &mut actual_format,
        &mut nitems,
        &mut bytes_after,
        &mut data,
    );
    if status != 0 || data.is_null() {
        return None;
    }
    let out = if actual_format == 32 {
        let longs = std::slice::from_raw_parts(data as *const c_ulong, nitems as usize);
        Some(longs.to_vec())
    } else {
        None
    };
    xlib::XFree(data as *mut _);
    out
}

/// Vier Ränder (links, rechts, oben, unten) aus einer Extents-Property.
unsafe fn get_extents(
    d: *mut xlib::Display,
    w: xlib::Window,
    prop: xlib::Atom,
) -> (i32, i32, i32, i32) {
    match get_u32_list(d, w, prop) {
        Some(v) if v.len() >= 4 => (v[0] as i32, v[1] as i32, v[2] as i32, v[3] as i32),
        _ => (0, 0, 0, 0),
    }
}

/// Position der linken oberen Ecke von `w` in Root-Koordinaten.
pub unsafe fn root_origin(d: *mut xlib::Display, w: xlib::Window) -> Option<(i32, i32)> {
    let root = xlib::XDefaultRootWindow(d);
    let mut rx: c_int = 0;
    let mut ry: c_int = 0;
    let mut child: xlib::Window = 0;
    if xlib::XTranslateCoordinates(d, w, root, 0, 0, &mut rx, &mut ry, &mut child) == 0 {
        return None;
    }
    Some((rx, ry))
}

/// Sichtbare fremde Fenster des aktuellen Arbeitsbereichs, unten → oben.
pub unsafe fn list_windows(d: *mut xlib::Display, a: &Atoms) -> Vec<WindowRect> {
    let root = xlib::XDefaultRootWindow(d);
    let own_pid = std::process::id() as u64;
    let current_desktop = get_u32_list(d, root, a.current_desktop).and_then(|v| v.first().copied());
    let Some(clients) = get_u32_list(d, root, a.client_list_stacking) else {
        return Vec::new();
    };

    let mut out = Vec::with_capacity(clients.len());
    for id in clients {
        let w = id as xlib::Window;

        // Eigene Overlays stehen ebenfalls in der Liste (normale gemanagte
        // Fenster) – sonst liefe das Schaf auf sich selbst.
        if get_u32_list(d, w, a.wm_pid).and_then(|v| v.first().copied()) == Some(own_pid) {
            continue;
        }
        if let (Some(cur), Some(dsk)) = (
            current_desktop,
            get_u32_list(d, w, a.wm_desktop).and_then(|v| v.first().copied()),
        ) {
            if dsk != ALL_DESKTOPS && dsk != cur {
                continue;
            }
        }
        // nemo-desktop (bildschirmfüllend, Oberkante y=0) und Panels.
        if let Some(types) = get_u32_list(d, w, a.wm_window_type) {
            if types
                .iter()
                .any(|&t| t == a.type_desktop || t == a.type_dock)
            {
                continue;
            }
        }
        if let Some(states) = get_u32_list(d, w, a.wm_state) {
            if states.contains(&a.state_hidden) {
                continue;
            }
        }

        let mut attrs: xlib::XWindowAttributes = std::mem::zeroed();
        if xlib::XGetWindowAttributes(d, w, &mut attrs) == 0 {
            continue; // inzwischen verschwunden
        }
        if attrs.map_state != xlib::IsViewable {
            continue;
        }
        let Some((cx, cy)) = root_origin(d, w) else {
            continue;
        };

        // Client-Bereich → um WM-Rahmen (Titelleiste) erweitern …
        let (fl, fr, ft, fb) = get_extents(d, w, a.frame_extents);
        // … und um unsichtbare CSD-Schattenränder (GTK-Apps) verkleinern.
        let (gl, gr, gt, gb) = get_extents(d, w, a.gtk_frame_extents);
        let rect = Rect {
            x: cx - fl + gl,
            y: cy - ft + gt,
            width: attrs.width + fl + fr - gl - gr,
            height: attrs.height + ft + fb - gt - gb,
        };
        if rect.width <= 0 || rect.height <= 0 {
            continue;
        }
        out.push(WindowRect { id, rect });
    }
    out
}

/// Kurzbeschreibung für die Debug-Ausgabe (`WM_NAME`, falls vorhanden).
pub unsafe fn window_name(d: *mut xlib::Display, w: xlib::Window) -> String {
    let mut name: *mut std::os::raw::c_char = std::ptr::null_mut();
    if xlib::XFetchName(d, w, &mut name) != 0 && !name.is_null() {
        let s = std::ffi::CStr::from_ptr(name)
            .to_string_lossy()
            .into_owned();
        xlib::XFree(name as *mut _);
        s
    } else {
        String::new()
    }
}
