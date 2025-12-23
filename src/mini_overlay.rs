//! Mini overlay module
//! Small, always-visible display showing remaining time

use std::mem::zeroed;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use windows::{
    core::w,
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, EndPaint, FillRect,
            InvalidateRect, SelectObject, SetBkMode, SetTextColor, DrawTextW,
            DT_CENTER, DT_SINGLELINE, DT_VCENTER, FW_BOLD, PAINTSTRUCT, TRANSPARENT,
        },
        UI::WindowsAndMessaging::*,
    },
};

use crate::blocking::REMAINING_SECONDS;
use crate::constants::*;

/// Global state for mini overlay window
pub static MINI_OVERLAY_HWND: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static MINI_OVERLAY_VISIBLE: AtomicBool = AtomicBool::new(false);

/// Timer ID for updating the mini overlay
pub const TIMER_MINI_UPDATE: usize = 10;

/// Mini overlay dimensions
const MINI_WIDTH: i32 = 140;
const MINI_HEIGHT: i32 = 36;
const MINI_MARGIN: i32 = 10;

/// Create the mini overlay window
pub unsafe fn create_mini_overlay(hinstance: windows::Win32::Foundation::HMODULE) {
    let class_name = w!("ScreenTimeMiniOverlayClass");

    let screen_width = GetSystemMetrics(SM_CXSCREEN);

    // Position in top-right corner
    let x = screen_width - MINI_WIDTH - MINI_MARGIN;
    let y = MINI_MARGIN;

    let ex_style = WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT;

    let hwnd = CreateWindowExW(
        ex_style,
        class_name,
        w!("Screen Time"),
        WS_POPUP,
        x,
        y,
        MINI_WIDTH,
        MINI_HEIGHT,
        None,
        None,
        hinstance,
        None,
    )
    .expect("Failed to create mini overlay window");

    // Set transparency (slightly see-through)
    SetLayeredWindowAttributes(hwnd, COLORREF(0), 200, LWA_ALPHA)
        .expect("Failed to set layered window attributes");

    MINI_OVERLAY_HWND.store(hwnd.0, Ordering::SeqCst);
}

/// Show the mini overlay and start the update timer
pub unsafe fn show_mini_overlay() {
    let hwnd = HWND(MINI_OVERLAY_HWND.load(Ordering::SeqCst));
    if hwnd.0.is_null() {
        return;
    }

    MINI_OVERLAY_VISIBLE.store(true, Ordering::SeqCst);

    let _ = InvalidateRect(hwnd, None, true);
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

    // Update every second
    let _ = SetTimer(hwnd, TIMER_MINI_UPDATE, 1000, None);
}

/// Hide the mini overlay
pub unsafe fn hide_mini_overlay() {
    let hwnd = HWND(MINI_OVERLAY_HWND.load(Ordering::SeqCst));
    if hwnd.0.is_null() {
        return;
    }

    MINI_OVERLAY_VISIBLE.store(false, Ordering::SeqCst);

    let _ = KillTimer(hwnd, TIMER_MINI_UPDATE);
    let _ = ShowWindow(hwnd, SW_HIDE);
}

/// Update the mini overlay display
pub unsafe fn update_mini_overlay() {
    let hwnd = HWND(MINI_OVERLAY_HWND.load(Ordering::SeqCst));
    if hwnd.0.is_null() {
        return;
    }

    let _ = InvalidateRect(hwnd, None, true);
}

/// Format seconds into a compact string (e.g., "1:30:45" or "30:45")
fn format_time_compact(seconds: i32) -> String {
    if seconds < 0 {
        return String::from("--:--");
    }

    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{}:{:02}", minutes, secs)
    }
}

/// Get color based on remaining time
fn get_time_color(seconds: i32) -> u32 {
    if seconds < 0 {
        COLOR_TEXT_LIGHT
    } else if seconds <= 60 {
        // Red when less than 1 minute
        0x004444FF
    } else if seconds <= 300 {
        // Orange when less than 5 minutes
        COLOR_ACCENT
    } else {
        // White otherwise
        COLOR_TEXT_WHITE
    }
}

/// Window procedure for the mini overlay
pub unsafe extern "system" fn mini_overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect: RECT = zeroed();
            GetClientRect(hwnd, &mut rect).ok();

            // Dark background with rounded feel
            let bg_brush = CreateSolidBrush(COLORREF(0x00222222));
            FillRect(hdc, &rect, bg_brush);
            let _ = DeleteObject(bg_brush);

            // Get remaining time
            let remaining = REMAINING_SECONDS.load(Ordering::SeqCst);
            let time_str = format_time_compact(remaining);
            let color = get_time_color(remaining);

            // Draw time
            let hfont = CreateFontW(
                22, 0, 0, 0,
                FW_BOLD.0 as i32,
                0, 0, 0, 0, 0, 0, 0, 0,
                w!("Consolas"),
            );

            let old_font = SelectObject(hdc, hfont);
            SetTextColor(hdc, COLORREF(color));
            SetBkMode(hdc, TRANSPARENT);

            let display_text = format!("{}", time_str);
            let wide_text: Vec<u16> = display_text.encode_utf16().collect();
            DrawTextW(
                hdc,
                &mut wide_text.clone(),
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            SelectObject(hdc, old_font);
            let _ = DeleteObject(hfont);

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TIMER_MINI_UPDATE {
                let current = REMAINING_SECONDS.load(Ordering::SeqCst);
                if current > 0 {
                    let new_time = current - 1;
                    REMAINING_SECONDS.store(new_time, Ordering::SeqCst);

                    // Save to database periodically (every 30 seconds)
                    if new_time % 30 == 0 {
                        crate::database::save_remaining_time(new_time);
                    }

                    // Trigger blocking overlay when time reaches 0
                    if new_time == 0 {
                        let msg = crate::database::get_blocking_message();
                        crate::blocking::show_blocking_overlay(&msg);
                    }
                }
                let _ = InvalidateRect(hwnd, None, true);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Register the mini overlay window class
pub unsafe fn register_mini_overlay_class(hinstance: windows::Win32::Foundation::HMODULE) {
    let class_name = w!("ScreenTimeMiniOverlayClass");
    let wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(mini_overlay_proc),
        hInstance: hinstance.into(),
        lpszClassName: class_name,
        hbrBackground: CreateSolidBrush(COLORREF(0x00222222)),
        ..zeroed()
    };

    if RegisterClassW(&wnd_class) == 0 {
        panic!("Failed to register mini overlay window class");
    }
}
