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

use crate::blocking::{hide_blocking_overlay, show_blocking_overlay, BLOCKING_HWND};
use crate::constants::*;
use crate::database::{get_blocking_message, get_warning_config};
use crate::dialogs::{show_settings_dialog, show_stats_dialog, verify_passcode_for_quit};
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

    InsertMenuW(hmenu, 0, MF_BYPOSITION | MF_STRING, IDM_TODAYS_STATS as usize, w!("Today's Stats..."))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 1, MF_BYPOSITION | MF_STRING, IDM_SETTINGS as usize, w!("Settings..."))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 2, MF_BYPOSITION | MF_SEPARATOR, 0, PCWSTR::null())
        .expect("Failed to insert separator");
    InsertMenuW(hmenu, 3, MF_BYPOSITION | MF_STRING, IDM_SHOW_OVERLAY as usize, w!("Show Warning (5s)"))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 4, MF_BYPOSITION | MF_STRING, IDM_SHOW_BLOCKING as usize, w!("Show Blocking Overlay"))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 5, MF_BYPOSITION | MF_SEPARATOR, 0, PCWSTR::null())
        .expect("Failed to insert separator");
    InsertMenuW(hmenu, 6, MF_BYPOSITION | MF_STRING, IDM_ABOUT as usize, w!("About"))
        .expect("Failed to insert menu item");
    InsertMenuW(hmenu, 7, MF_BYPOSITION | MF_STRING, IDM_QUIT as usize, w!("Quit"))
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
