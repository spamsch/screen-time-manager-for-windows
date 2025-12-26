//! System tray module for Screen Time Manager
//! Handles the system tray icon and context menu

use std::mem::zeroed;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::{
                Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
                NOTIFYICONDATAW,
            },
            WindowsAndMessaging::*,
        },
    },
};

use crate::blocking::{extend_time, hide_blocking_overlay, show_blocking_overlay, BLOCKING_HWND};
use crate::constants::*;
use crate::database::{get_blocking_message, get_warning_config, is_pause_enabled};
use crate::dialogs::{show_settings_dialog, show_stats_dialog, verify_passcode_for_quit};
use crate::mini_overlay::{is_paused, can_pause, toggle_pause, PauseBlockedReason, get_remaining_pause_budget};
use crate::overlay::{show_overlay, OVERLAY_HWND};
use std::sync::atomic::Ordering;

/// Global state for the notification icon data
pub static mut NOTIFY_ICON_DATA: Option<NOTIFYICONDATAW> = None;

/// Add the system tray icon
pub unsafe fn add_tray_icon(hwnd: HWND) {
    let hinstance = GetModuleHandleW(None).expect("Failed to get module handle");

    let hicon = LoadIconW(hinstance, PCWSTR(1 as *const u16))
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .expect("Failed to load icon");

    let tooltip = "Screen Time Manager";
    let mut tip_buffer: [u16; 128] = [0; 128];
    for (i, c) in tooltip.encode_utf16().enumerate() {
        if i >= 127 { break; }
        tip_buffer[i] = c;
    }

    let mut nid: NOTIFYICONDATAW = zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = hicon;
    nid.szTip = tip_buffer;

    if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
        panic!("Failed to add tray icon");
    }

    NOTIFY_ICON_DATA = Some(nid);
}

/// Remove the system tray icon
pub unsafe fn remove_tray_icon() {
    if let Some(ref nid) = NOTIFY_ICON_DATA {
        let _ = Shell_NotifyIconW(NIM_DELETE, nid);
        NOTIFY_ICON_DATA = None;
    }
}

/// Show the context menu when right-clicking the tray icon
pub unsafe fn show_context_menu(hwnd: HWND) {
    let hmenu = CreatePopupMenu().expect("Failed to create popup menu");

    // Determine pause menu item text and state
    let paused = is_paused();
    let pause_enabled = is_pause_enabled();

    let (pause_text, pause_flags) = if paused {
        // Currently paused - show resume option (always available)
        ("Resume Timer", MF_BYPOSITION | MF_STRING)
    } else if !pause_enabled {
        // Pause feature disabled
        ("Pause (Disabled)", MF_BYPOSITION | MF_STRING | MF_GRAYED)
    } else {
        // Check if pause is available
        match can_pause() {
            Ok(()) => {
                let budget_mins = get_remaining_pause_budget() / 60;
                let text = format!("Pause Timer ({}m left)", budget_mins);
                // Need to leak the string for the menu (will be cleaned up with menu)
                let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let ptr = wide.as_ptr();
                std::mem::forget(wide);
                return show_context_menu_with_pause(hwnd, hmenu, PCWSTR(ptr), MF_BYPOSITION | MF_STRING);
            }
            Err(PauseBlockedReason::BudgetExhausted) => {
                ("Pause (Budget used)", MF_BYPOSITION | MF_STRING | MF_GRAYED)
            }
            Err(PauseBlockedReason::CooldownActive { seconds_remaining }) => {
                let mins = (seconds_remaining + 59) / 60; // Round up
                let text = format!("Pause ({}m cooldown)", mins);
                let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let ptr = wide.as_ptr();
                std::mem::forget(wide);
                return show_context_menu_with_pause(hwnd, hmenu, PCWSTR(ptr), MF_BYPOSITION | MF_STRING | MF_GRAYED);
            }
            Err(PauseBlockedReason::MinActiveTimeNotMet { seconds_remaining }) => {
                let mins = (seconds_remaining + 59) / 60;
                let text = format!("Pause (wait {}m)", mins);
                let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let ptr = wide.as_ptr();
                std::mem::forget(wide);
                return show_context_menu_with_pause(hwnd, hmenu, PCWSTR(ptr), MF_BYPOSITION | MF_STRING | MF_GRAYED);
            }
            Err(PauseBlockedReason::TimeTooLow) => {
                ("Pause (Time too low)", MF_BYPOSITION | MF_STRING | MF_GRAYED)
            }
            Err(PauseBlockedReason::Disabled) => {
                ("Pause (Disabled)", MF_BYPOSITION | MF_STRING | MF_GRAYED)
            }
        }
    };

    let pause_wide: Vec<u16> = pause_text.encode_utf16().chain(std::iter::once(0)).collect();
    show_context_menu_with_pause(hwnd, hmenu, PCWSTR(pause_wide.as_ptr()), pause_flags);
}

/// Helper to show context menu with pause item
unsafe fn show_context_menu_with_pause(hwnd: HWND, hmenu: HMENU, pause_text: PCWSTR, pause_flags: MENU_ITEM_FLAGS) {
    InsertMenuW(hmenu, 0, MF_BYPOSITION | MF_STRING, IDM_TODAYS_STATS as usize, w!("Today's Stats..."))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 1, MF_BYPOSITION | MF_STRING, IDM_SETTINGS as usize, w!("Settings..."))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 2, MF_BYPOSITION | MF_SEPARATOR, 0, PCWSTR::null())
        .expect("Failed to insert separator");
    InsertMenuW(hmenu, 3, MF_BYPOSITION | MF_STRING, IDM_EXTEND_15 as usize, w!("Extend +15 min"))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 4, MF_BYPOSITION | MF_STRING, IDM_EXTEND_45 as usize, w!("Extend +45 min"))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 5, MF_BYPOSITION | MF_SEPARATOR, 0, PCWSTR::null())
        .expect("Failed to insert separator");

    // Pause menu item with dynamic text
    InsertMenuW(hmenu, 6, pause_flags, IDM_PAUSE_TOGGLE as usize, pause_text)
        .expect("Failed to insert pause menu item");

    InsertMenuW(hmenu, 7, MF_BYPOSITION | MF_SEPARATOR, 0, PCWSTR::null())
        .expect("Failed to insert separator");
    InsertMenuW(hmenu, 8, MF_BYPOSITION | MF_STRING, IDM_SHOW_OVERLAY as usize, w!("Show Warning (5s)"))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 9, MF_BYPOSITION | MF_STRING, IDM_SHOW_BLOCKING as usize, w!("Show Blocking Overlay"))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 10, MF_BYPOSITION | MF_SEPARATOR, 0, PCWSTR::null())
        .expect("Failed to insert separator");
    InsertMenuW(hmenu, 11, MF_BYPOSITION | MF_STRING, IDM_ABOUT as usize, w!("About"))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 12, MF_BYPOSITION | MF_STRING, IDM_QUIT as usize, w!("Quit"))
        .expect("Failed to insert menu item");

    let mut point = zeroed();
    GetCursorPos(&mut point).expect("Failed to get cursor position");

    let _ = SetForegroundWindow(hwnd);

    let _ = TrackPopupMenu(
        hmenu,
        TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
        point.x,
        point.y,
        0,
        hwnd,
        None,
    );

    DestroyMenu(hmenu).ok();
}

/// Main window procedure for handling tray events
pub unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = lparam.0 as u32;
            match event {
                WM_RBUTTONUP | WM_LBUTTONUP => {
                    show_context_menu(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let menu_id = (wparam.0 & 0xFFFF) as u16;
            match menu_id {
                IDM_PAUSE_TOGGLE => {
                    // Toggle pause state (no passcode required - it's a child feature)
                    match toggle_pause() {
                        Ok(_is_now_paused) => {
                            // Success - UI will update automatically
                        }
                        Err(_reason) => {
                            // Should not happen since menu item should be grayed out
                            // But just in case, do nothing
                        }
                    }
                }
                IDM_SHOW_OVERLAY => {
                    let (minutes, message) = get_warning_config(1);
                    show_overlay(&message, minutes);
                }
                IDM_SHOW_BLOCKING => {
                    let message = get_blocking_message();
                    show_blocking_overlay(&message);
                }
                IDM_TODAYS_STATS => {
                    if verify_passcode_for_quit(hwnd) {
                        show_stats_dialog(hwnd);
                    }
                }
                IDM_SETTINGS => {
                    if verify_passcode_for_quit(hwnd) {
                        show_settings_dialog(hwnd);
                    }
                }
                IDM_EXTEND_15 => {
                    if verify_passcode_for_quit(hwnd) {
                        extend_time(15);
                    }
                }
                IDM_EXTEND_45 => {
                    if verify_passcode_for_quit(hwnd) {
                        extend_time(45);
                    }
                }
                IDM_ABOUT => {
                    MessageBoxW(
                        hwnd,
                        w!("Screen Time Manager v0.1.0\n\nA parental control application for managing screen time."),
                        w!("About"),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
                IDM_QUIT => {
                    if verify_passcode_for_quit(hwnd) {
                        DestroyWindow(hwnd).ok();
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let overlay_hwnd = HWND(OVERLAY_HWND.load(Ordering::SeqCst));
            if !overlay_hwnd.0.is_null() {
                DestroyWindow(overlay_hwnd).ok();
            }
            let blocking_hwnd = HWND(BLOCKING_HWND.load(Ordering::SeqCst));
            if !blocking_hwnd.0.is_null() {
                hide_blocking_overlay();
                DestroyWindow(blocking_hwnd).ok();
            }
            remove_tray_icon();
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
