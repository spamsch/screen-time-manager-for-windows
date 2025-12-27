//! Screen Time Manager - A system tray application for Windows
//!
//! This application runs in the background with only a system tray icon visible.
//! Right-clicking the icon shows a context menu with options including quit.

#![windows_subsystem = "windows"]

mod blocking;
mod constants;
mod database;
mod dialogs;
mod mini_overlay;
mod overlay;
mod telegram;
mod tray;

use std::mem::zeroed;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{BOOL, GetLastError, CloseHandle, ERROR_ALREADY_EXISTS},
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::CreateMutexW,
        },
        UI::WindowsAndMessaging::*,
    },
};

use blocking::{create_blocking_overlay, create_secondary_overlays, register_blocking_class, REMAINING_SECONDS};
use constants::MUTEX_NAME;
use database::{init_database, load_remaining_time, get_current_weekday, get_daily_limit};
use mini_overlay::{create_mini_overlay, register_mini_overlay_class, show_mini_overlay};
use overlay::{create_overlay_window, register_overlay_class};
use tray::{add_tray_icon, remove_tray_icon, window_proc};
use std::sync::atomic::Ordering;

fn main() {
    unsafe {
        // Check for single instance
        if !ensure_single_instance() {
            MessageBoxW(
                None,
                w!("Screen Time Manager is already running."),
                w!("Already Running"),
                MB_OK | MB_ICONWARNING,
            );
            return;
        }

        // Initialize database
        if let Err(e) = init_database() {
            let msg: Vec<u16> = format!("Failed to initialize database: {}\0", e)
                .encode_utf16()
                .collect();
            MessageBoxW(
                None,
                PCWSTR(msg.as_ptr()),
                w!("Database Error"),
                MB_OK | MB_ICONERROR,
            );
            return;
        }

        // Get the module handle
        let hinstance = GetModuleHandleW(None).expect("Failed to get module handle");

        // Register main window class
        let class_name = w!("ScreenTimeManagerClass");
        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..zeroed()
        };

        if RegisterClassW(&wnd_class) == 0 {
            panic!("Failed to register window class");
        }

        // Register overlay and blocking window classes
        register_overlay_class(hinstance);
        register_blocking_class(hinstance);
        register_mini_overlay_class(hinstance);

        // Create a hidden window for message handling
        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            w!("Screen Time Manager"),
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            hinstance,
            None,
        )
        .expect("Failed to create window");

        // Create the overlay windows (initially hidden)
        create_overlay_window(hinstance);
        create_blocking_overlay(hinstance);
        create_secondary_overlays(hinstance);  // Create overlays for secondary monitors
        create_mini_overlay(hinstance);

        // Initialize remaining time from database or daily limit
        let remaining = load_remaining_time().unwrap_or_else(|| {
            // No saved time for today, use daily limit
            let weekday = get_current_weekday();
            (get_daily_limit(weekday) * 60) as i32  // Convert minutes to seconds
        });
        REMAINING_SECONDS.store(remaining, Ordering::SeqCst);

        // Initialize session active time from database
        let session_active = database::get_session_active_time();
        mini_overlay::SESSION_ACTIVE_SECONDS.store(session_active, Ordering::SeqCst);

        // Show the mini overlay with remaining time
        show_mini_overlay();

        // If time is already exhausted, show blocking overlay immediately
        if remaining <= 0 {
            let msg = database::get_blocking_message();
            blocking::show_blocking_overlay(&msg);
        }

        // Add the system tray icon
        add_tray_icon(hwnd);

        // Start Telegram bot in background thread (if configured)
        telegram::start_bot_thread();

        // Message loop
        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Cleanup: remove the tray icon
        remove_tray_icon();
    }
}

/// Ensures only one instance of the application is running
unsafe fn ensure_single_instance() -> bool {
    let mutex_name: Vec<u16> = MUTEX_NAME.encode_utf16().chain(std::iter::once(0)).collect();

    let handle = CreateMutexW(
        None,
        BOOL::from(true),
        PCWSTR(mutex_name.as_ptr()),
    );

    match handle {
        Ok(h) => {
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(h);
                false
            } else {
                true
            }
        }
        Err(_) => false,
    }
}
