//! DPI scaling helper module
//! Provides utilities for DPI-aware UI scaling

use std::sync::atomic::{AtomicU32, Ordering};
use windows::Win32::UI::HiDpi::GetDpiForSystem;

/// Cached DPI value (0 means not initialized)
static CACHED_DPI: AtomicU32 = AtomicU32::new(0);

/// Standard DPI (96 = 100% scaling)
const STANDARD_DPI: u32 = 96;

/// Initialize and cache the system DPI value
/// Should be called once at startup after setting DPI awareness
pub fn init_dpi() {
    let dpi = unsafe { GetDpiForSystem() };
    CACHED_DPI.store(dpi, Ordering::SeqCst);
}

/// Get the current DPI value
pub fn get_dpi() -> u32 {
    let cached = CACHED_DPI.load(Ordering::SeqCst);
    if cached == 0 {
        // Fallback: get DPI if not initialized
        let dpi = unsafe { GetDpiForSystem() };
        CACHED_DPI.store(dpi, Ordering::SeqCst);
        dpi
    } else {
        cached
    }
}

/// Get the scale factor as a ratio (1.0 = 100%, 1.5 = 150%, etc.)
pub fn get_scale_factor() -> f32 {
    get_dpi() as f32 / STANDARD_DPI as f32
}

/// Scale an integer value by the DPI factor
pub fn scale(value: i32) -> i32 {
    let dpi = get_dpi();
    // Use MulDiv-style calculation to avoid floating point
    ((value as i64 * dpi as i64 + STANDARD_DPI as i64 / 2) / STANDARD_DPI as i64) as i32
}

/// Scale a font size (similar to scale but for font heights)
pub fn scale_font(size: i32) -> i32 {
    scale(size)
}
