//! Constants module for Screen Time Manager
//! Contains all shared constants used across the application

// Custom message ID for tray icon events
pub const WM_TRAYICON: u32 = 0x8001;

// Menu item IDs
pub const IDM_ABOUT: u16 = 1001;
pub const IDM_QUIT: u16 = 1002;
pub const IDM_SHOW_OVERLAY: u16 = 1003;
pub const IDM_SHOW_BLOCKING: u16 = 1004;
pub const IDM_SETTINGS: u16 = 1005;
pub const IDM_TODAYS_STATS: u16 = 1006;
pub const IDM_PAUSE_TOGGLE: u16 = 1007;

// Mutex name for single instance
pub const MUTEX_NAME: &str = "Global\\ScreenTimeManager_SingleInstance_7F3A9B2E";

// Colors (BGR format)
pub const COLOR_OVERLAY_BG: u32 = 0x00331a00;      // Dark blue-ish
pub const COLOR_PANEL_BG: u32 = 0x00442200;        // Slightly lighter
pub const COLOR_ACCENT: u32 = 0x00ff9933;          // Orange accent
pub const COLOR_TEXT_WHITE: u32 = 0x00FFFFFF;
pub const COLOR_TEXT_LIGHT: u32 = 0x00CCCCCC;
pub const COLOR_ERROR: u32 = 0x004444FF;           // Red (BGR)
