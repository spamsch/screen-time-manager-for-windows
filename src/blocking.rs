//! Blocking overlay module
//! Full-screen overlay that requires passcode to dismiss

use std::mem::zeroed;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering};
use std::sync::Mutex;
use windows::{
    core::w,
    Win32::{
        Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW,
            EndPaint, EnumDisplayMonitors, FillRect, InvalidateRect, RoundRect, SelectObject,
            SetBkMode, SetTextColor, DT_CENTER, DT_SINGLELINE, DT_VCENTER, FW_BOLD, FW_NORMAL,
            HDC, HMONITOR, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
        },
        Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC},
        System::LibraryLoader::GetModuleHandleW,
        System::Shutdown::{ExitWindowsEx, EWX_SHUTDOWN, SHUTDOWN_REASON},
        UI::{
            Controls::*,
            Input::KeyboardAndMouse::{SetFocus, VK_RETURN},
            WindowsAndMessaging::*,
        },
    },
};

use crate::constants::*;
use crate::database::get_passcode;

/// Storage for secondary monitor overlay handles (stores raw pointers as isize for Send+Sync)
static SECONDARY_OVERLAY_HWNDS: Mutex<Vec<isize>> = Mutex::new(Vec::new());

/// Monitor information collected during enumeration
struct MonitorInfo {
    rect: RECT,
    is_primary: bool,
}

/// Global state for blocking overlay
pub static BLOCKING_HWND: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static BLOCKING_TEXT: Mutex<Option<String>> = Mutex::new(None);
pub static BLOCKING_EDIT_HWND: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static PASSCODE_ERROR: AtomicBool = AtomicBool::new(false);

/// Remaining time in seconds (negative means no limit/extension active)
pub static REMAINING_SECONDS: AtomicI32 = AtomicI32::new(-1);

/// Get remaining time in seconds
pub fn get_remaining_seconds() -> i32 {
    REMAINING_SECONDS.load(Ordering::SeqCst)
}

/// Timer IDs
pub const TIMER_REASSERT_TOPMOST: usize = 2;
pub const TIMER_COUNTDOWN: usize = 3;

/// Control IDs
const ID_PASSCODE_EDIT: i32 = 101;
const ID_UNLOCK_BUTTON: i32 = 102;
const ID_EXTEND_15: i32 = 103;
const ID_EXTEND_30: i32 = 104;
const ID_EXTEND_60: i32 = 105;
const ID_SHUTDOWN_BUTTON: i32 = 106;

pub unsafe fn create_blocking_overlay(hinstance: windows::Win32::Foundation::HMODULE) {
    let class_name = w!("ScreenTimeBlockingClass");

    let screen_width = GetSystemMetrics(SM_CXSCREEN);
    let screen_height = GetSystemMetrics(SM_CYSCREEN);

    let ex_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW;

    let hwnd = CreateWindowExW(
        ex_style,
        class_name,
        w!("Screen Time - Time's Up!"),
        WS_POPUP,
        0,
        0,
        screen_width,
        screen_height,
        None,
        None,
        hinstance,
        None,
    )
    .expect("Failed to create blocking overlay");

    BLOCKING_HWND.store(hwnd.0, Ordering::SeqCst);
}

/// Shows the full-screen blocking overlay
pub unsafe fn show_blocking_overlay(text: &str) {
    show_blocking_overlay_with_time(text, -1);
}

/// Shows the full-screen blocking overlay with optional remaining time in seconds
pub unsafe fn show_blocking_overlay_with_time(text: &str, remaining_seconds: i32) {
    let hwnd = HWND(BLOCKING_HWND.load(Ordering::SeqCst));
    if hwnd.0.is_null() {
        return;
    }

    // Hide the mini overlay while blocking screen is shown
    crate::mini_overlay::hide_mini_overlay();

    *BLOCKING_TEXT.lock().unwrap() = Some(text.to_string());
    PASSCODE_ERROR.store(false, Ordering::SeqCst);
    if remaining_seconds >= 0 {
        REMAINING_SECONDS.store(remaining_seconds, Ordering::SeqCst);
    }

    let edit_ptr = BLOCKING_EDIT_HWND.load(Ordering::SeqCst);
    if !edit_ptr.is_null() {
        SetWindowTextW(HWND(edit_ptr), w!("")).ok();
    }

    let _ = InvalidateRect(hwnd, None, false);

    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        0, 0, 0, 0,
        SWP_SHOWWINDOW | SWP_NOMOVE | SWP_NOSIZE,
    ).ok();

    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = SetForegroundWindow(hwnd);

    let edit_ptr = BLOCKING_EDIT_HWND.load(Ordering::SeqCst);
    if !edit_ptr.is_null() {
        let _ = SetFocus(HWND(edit_ptr));
    }

    let _ = PlaySoundW(w!("SystemHand"), None, SND_ALIAS | SND_ASYNC);
    let _ = SetTimer(hwnd, TIMER_REASSERT_TOPMOST, 500, None);

    // Start countdown timer (updates every second)
    let _ = SetTimer(hwnd, TIMER_COUNTDOWN, 1000, None);

    // Show secondary monitor overlays (blanks other monitors)
    show_secondary_overlays();
}

/// Extend the remaining time by the specified minutes
pub fn extend_time(minutes: i32) {
    let current = REMAINING_SECONDS.load(Ordering::SeqCst);
    let additional_seconds = minutes * 60;

    if current < 0 {
        // No timer was running, start fresh
        REMAINING_SECONDS.store(additional_seconds, Ordering::SeqCst);
    } else {
        // Add to existing time
        REMAINING_SECONDS.store(current + additional_seconds, Ordering::SeqCst);
    }
}

/// Format seconds into a human-readable string (e.g., "1h 30m 45s")
fn format_time(seconds: i32) -> String {
    if seconds < 0 {
        return String::from("--:--");
    }

    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Hides the blocking overlay
pub unsafe fn hide_blocking_overlay() {
    let hwnd = HWND(BLOCKING_HWND.load(Ordering::SeqCst));
    if hwnd.0.is_null() {
        return;
    }

    let _ = KillTimer(hwnd, TIMER_REASSERT_TOPMOST);
    let _ = KillTimer(hwnd, TIMER_COUNTDOWN);
    let _ = ShowWindow(hwnd, SW_HIDE);
    *BLOCKING_TEXT.lock().unwrap() = None;

    // Hide secondary monitor overlays
    hide_secondary_overlays();

    // Save remaining time to database
    let remaining = REMAINING_SECONDS.load(Ordering::SeqCst);
    crate::database::save_remaining_time(remaining);

    // Show mini overlay again if there's remaining time
    if remaining > 0 {
        crate::mini_overlay::show_mini_overlay();
    }
}

/// Verify passcode entered in blocking overlay
unsafe fn check_blocking_passcode() -> bool {
    let edit_ptr = BLOCKING_EDIT_HWND.load(Ordering::SeqCst);
    if !edit_ptr.is_null() {
        let edit = HWND(edit_ptr);
        let mut buffer = [0u16; 16];
        let len = GetWindowTextW(edit, &mut buffer);
        let entered: String = String::from_utf16_lossy(&buffer[..len as usize]);

        if let Some(stored) = get_passcode() {
            if entered == stored {
                return true;
            }
        }
    }
    false
}

pub unsafe extern "system" fn blocking_overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let hinstance = GetModuleHandleW(None).unwrap();
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);

            // Expanded panel dimensions
            let panel_width = 500;
            let panel_height = 530;
            let _panel_x = (screen_width - panel_width) / 2;
            let panel_y = (screen_height - panel_height) / 2;

            // Button font for extend buttons
            let btn_font = CreateFontW(
                18, 0, 0, 0,
                FW_BOLD.0 as i32,
                0, 0, 0, 0, 0, 0, 0, 0,
                w!("Segoe UI"),
            );

            // Extend time buttons (in a row)
            let extend_btn_width = 100;
            let extend_btn_height = 40;
            let extend_y = panel_y + 200;
            let extend_spacing = 20;
            let total_extend_width = extend_btn_width * 3 + extend_spacing * 2;
            let extend_start_x = (screen_width - total_extend_width) / 2;

            // +15 min button
            let btn_15 = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("+15 min"),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                extend_start_x,
                extend_y,
                extend_btn_width,
                extend_btn_height,
                hwnd,
                HMENU(ID_EXTEND_15 as _),
                hinstance,
                None,
            );
            if let Ok(h) = btn_15 {
                SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1));
            }

            // +30 min button
            let btn_30 = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("+30 min"),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                extend_start_x + extend_btn_width + extend_spacing,
                extend_y,
                extend_btn_width,
                extend_btn_height,
                hwnd,
                HMENU(ID_EXTEND_30 as _),
                hinstance,
                None,
            );
            if let Ok(h) = btn_30 {
                SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1));
            }

            // +60 min button
            let btn_60 = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("+60 min"),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                extend_start_x + (extend_btn_width + extend_spacing) * 2,
                extend_y,
                extend_btn_width,
                extend_btn_height,
                hwnd,
                HMENU(ID_EXTEND_60 as _),
                hinstance,
                None,
            );
            if let Ok(h) = btn_60 {
                SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1));
            }

            // Passcode edit control
            let edit_width = 200;
            let edit_height = 50;
            let edit_x = (screen_width - edit_width) / 2;
            let edit_y = panel_y + 310;

            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_BORDER
                    | WINDOW_STYLE(ES_CENTER as u32 | ES_PASSWORD as u32 | ES_NUMBER as u32),
                edit_x,
                edit_y,
                edit_width,
                edit_height,
                hwnd,
                HMENU(ID_PASSCODE_EDIT as _),
                hinstance,
                None,
            ).ok();

            if let Some(e) = edit {
                BLOCKING_EDIT_HWND.store(e.0, Ordering::SeqCst);
                SendMessageW(e, EM_SETLIMITTEXT, WPARAM(4), LPARAM(0));

                let hfont = CreateFontW(
                    32, 0, 0, 0,
                    FW_BOLD.0 as i32,
                    0, 0, 0, 0, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                SendMessageW(e, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
            }

            // Unlock button
            let btn_width = 200;
            let btn_height = 45;
            let btn_x = (screen_width - btn_width) / 2;
            let btn_y = edit_y + edit_height + 15;

            let _ = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("Unlock"),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                btn_x,
                btn_y,
                btn_width,
                btn_height,
                hwnd,
                HMENU(ID_UNLOCK_BUTTON as _),
                hinstance,
                None,
            );

            // Shutdown button
            let shutdown_btn_y = btn_y + btn_height + 15;
            let shutdown_btn = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("Shut Down"),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                btn_x,
                shutdown_btn_y,
                btn_width,
                btn_height,
                hwnd,
                HMENU(ID_SHUTDOWN_BUTTON as _),
                hinstance,
                None,
            );
            if let Ok(h) = shutdown_btn {
                SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1));
            }

            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect: RECT = zeroed();
            GetClientRect(hwnd, &mut rect).ok();

            let bg_brush = CreateSolidBrush(COLORREF(COLOR_OVERLAY_BG));
            FillRect(hdc, &rect, bg_brush);
            let _ = DeleteObject(bg_brush);

            let screen_width = rect.right;
            let screen_height = rect.bottom;

            // Expanded panel with more margin
            let panel_width = 500;
            let panel_height = 530;
            let panel_x = (screen_width - panel_width) / 2;
            let panel_y = (screen_height - panel_height) / 2;

            let panel_brush = CreateSolidBrush(COLORREF(COLOR_PANEL_BG));
            let old_brush = SelectObject(hdc, panel_brush);
            let pen = CreatePen(PS_SOLID, 2, COLORREF(COLOR_ACCENT));
            let old_pen = SelectObject(hdc, pen);

            let _ = RoundRect(hdc, panel_x, panel_y, panel_x + panel_width, panel_y + panel_height, 20, 20);

            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
            let _ = DeleteObject(panel_brush);
            let _ = DeleteObject(pen);

            // Title
            let title_font = CreateFontW(
                42, 0, 0, 0,
                FW_BOLD.0 as i32,
                0, 0, 0, 0, 0, 0, 0, 0,
                w!("Segoe UI"),
            );
            let old_font = SelectObject(hdc, title_font);
            SetTextColor(hdc, COLORREF(COLOR_TEXT_WHITE));
            SetBkMode(hdc, TRANSPARENT);

            let mut title_rect = RECT {
                left: panel_x,
                top: panel_y + 25,
                right: panel_x + panel_width,
                bottom: panel_y + 75,
            };
            DrawTextW(
                hdc,
                &mut "Time's Up!".encode_utf16().collect::<Vec<_>>(),
                &mut title_rect,
                DT_CENTER | DT_SINGLELINE,
            );

            // Remaining time display
            let remaining = REMAINING_SECONDS.load(Ordering::SeqCst);
            let time_font = CreateFontW(
                36, 0, 0, 0,
                FW_BOLD.0 as i32,
                0, 0, 0, 0, 0, 0, 0, 0,
                w!("Segoe UI"),
            );
            SelectObject(hdc, time_font);
            SetTextColor(hdc, COLORREF(COLOR_ACCENT));

            let time_str = if remaining >= 0 {
                format!("Remaining: {}", format_time(remaining))
            } else {
                String::from("Time limit exceeded")
            };
            let mut time_rect = RECT {
                left: panel_x,
                top: panel_y + 80,
                right: panel_x + panel_width,
                bottom: panel_y + 120,
            };
            let wide_time: Vec<u16> = time_str.encode_utf16().collect();
            DrawTextW(
                hdc,
                &mut wide_time.clone(),
                &mut time_rect,
                DT_CENTER | DT_SINGLELINE,
            );

            // Message
            let msg_font = CreateFontW(
                20, 0, 0, 0,
                FW_NORMAL.0 as i32,
                0, 0, 0, 0, 0, 0, 0, 0,
                w!("Segoe UI"),
            );
            SelectObject(hdc, msg_font);
            SetTextColor(hdc, COLORREF(COLOR_TEXT_LIGHT));

            let blocking_text_guard = BLOCKING_TEXT.lock().unwrap();
            let message = blocking_text_guard.as_ref().map(|s| s.as_str()).unwrap_or("Screen time limit reached");
            let mut msg_rect = RECT {
                left: panel_x + 30,
                top: panel_y + 125,
                right: panel_x + panel_width - 30,
                bottom: panel_y + 160,
            };
            let wide_msg: Vec<u16> = message.encode_utf16().collect();
            DrawTextW(
                hdc,
                &mut wide_msg.clone(),
                &mut msg_rect,
                DT_CENTER | DT_SINGLELINE,
            );
            drop(blocking_text_guard);

            // "Extend time:" label
            let label_font = CreateFontW(
                16, 0, 0, 0,
                FW_NORMAL.0 as i32,
                0, 0, 0, 0, 0, 0, 0, 0,
                w!("Segoe UI"),
            );
            SelectObject(hdc, label_font);
            SetTextColor(hdc, COLORREF(COLOR_TEXT_LIGHT));

            let mut extend_label_rect = RECT {
                left: panel_x,
                top: panel_y + 170,
                right: panel_x + panel_width,
                bottom: panel_y + 190,
            };
            DrawTextW(
                hdc,
                &mut "Extend time (requires passcode):".encode_utf16().collect::<Vec<_>>(),
                &mut extend_label_rect,
                DT_CENTER | DT_SINGLELINE,
            );

            // "Enter passcode to unlock:" label
            let mut passcode_label_rect = RECT {
                left: panel_x,
                top: panel_y + 260,
                right: panel_x + panel_width,
                bottom: panel_y + 285,
            };
            DrawTextW(
                hdc,
                &mut "Enter passcode to unlock:".encode_utf16().collect::<Vec<_>>(),
                &mut passcode_label_rect,
                DT_CENTER | DT_SINGLELINE,
            );

            // Error message
            if PASSCODE_ERROR.load(Ordering::SeqCst) {
                SetTextColor(hdc, COLORREF(COLOR_ERROR));
                let mut error_rect = RECT {
                    left: panel_x,
                    top: panel_y + panel_height - 45,
                    right: panel_x + panel_width,
                    bottom: panel_y + panel_height - 20,
                };
                DrawTextW(
                    hdc,
                    &mut "Incorrect passcode!".encode_utf16().collect::<Vec<_>>(),
                    &mut error_rect,
                    DT_CENTER | DT_SINGLELINE,
                );
            }

            SelectObject(hdc, old_font);
            let _ = DeleteObject(title_font);
            let _ = DeleteObject(time_font);
            let _ = DeleteObject(msg_font);
            let _ = DeleteObject(label_font);

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let notification = ((wparam.0 >> 16) & 0xFFFF) as u32;

            if notification == BN_CLICKED as u32 {
                match id {
                    ID_UNLOCK_BUTTON => {
                        if check_blocking_passcode() {
                            hide_blocking_overlay();
                        } else {
                            PASSCODE_ERROR.store(true, Ordering::SeqCst);
                            let _ = InvalidateRect(hwnd, None, false);
                            let edit_ptr = BLOCKING_EDIT_HWND.load(Ordering::SeqCst);
                            if !edit_ptr.is_null() {
                                let edit = HWND(edit_ptr);
                                SetWindowTextW(edit, w!("")).ok();
                                let _ = SetFocus(edit);
                            }
                            let _ = PlaySoundW(w!("SystemExclamation"), None, SND_ALIAS | SND_ASYNC);
                        }
                    }
                    ID_EXTEND_15 | ID_EXTEND_30 | ID_EXTEND_60 => {
                        // Require passcode for extension
                        if check_blocking_passcode() {
                            let minutes = match id {
                                ID_EXTEND_15 => 15,
                                ID_EXTEND_30 => 30,
                                ID_EXTEND_60 => 60,
                                _ => 0,
                            };
                            extend_time(minutes);
                            PASSCODE_ERROR.store(false, Ordering::SeqCst);

                            // Clear the passcode field
                            let edit_ptr = BLOCKING_EDIT_HWND.load(Ordering::SeqCst);
                            if !edit_ptr.is_null() {
                                SetWindowTextW(HWND(edit_ptr), w!("")).ok();
                            }

                            // Hide overlay and let the user continue
                            hide_blocking_overlay();
                        } else {
                            PASSCODE_ERROR.store(true, Ordering::SeqCst);
                            let _ = InvalidateRect(hwnd, None, false);
                            let edit_ptr = BLOCKING_EDIT_HWND.load(Ordering::SeqCst);
                            if !edit_ptr.is_null() {
                                let edit = HWND(edit_ptr);
                                SetWindowTextW(edit, w!("")).ok();
                                let _ = SetFocus(edit);
                            }
                            let _ = PlaySoundW(w!("SystemExclamation"), None, SND_ALIAS | SND_ASYNC);
                        }
                    }
                    ID_SHUTDOWN_BUTTON => {
                        // Require passcode for shutdown
                        if check_blocking_passcode() {
                            // Initiate system shutdown
                            let _ = ExitWindowsEx(EWX_SHUTDOWN, SHUTDOWN_REASON(0));
                        } else {
                            PASSCODE_ERROR.store(true, Ordering::SeqCst);
                            let _ = InvalidateRect(hwnd, None, false);
                            let edit_ptr = BLOCKING_EDIT_HWND.load(Ordering::SeqCst);
                            if !edit_ptr.is_null() {
                                let edit = HWND(edit_ptr);
                                SetWindowTextW(edit, w!("")).ok();
                                let _ = SetFocus(edit);
                            }
                            let _ = PlaySoundW(w!("SystemExclamation"), None, SND_ALIAS | SND_ASYNC);
                        }
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            match wparam.0 {
                TIMER_REASSERT_TOPMOST => {
                    SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        0, 0, 0, 0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    ).ok();
                }
                TIMER_COUNTDOWN => {
                    // Just redraw to update time display (mini overlay handles countdown)
                    // Use false to avoid erasing background (prevents flickering)
                    let _ = InvalidateRect(hwnd, None, false);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => {
            // Return non-zero to indicate we handle background erasing (prevents flickering)
            LRESULT(1)
        }
        WM_CLOSE => {
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 == VK_RETURN.0 as usize {
                if check_blocking_passcode() {
                    hide_blocking_overlay();
                } else {
                    PASSCODE_ERROR.store(true, Ordering::SeqCst);
                    let _ = InvalidateRect(hwnd, None, false);
                    let edit_ptr = BLOCKING_EDIT_HWND.load(Ordering::SeqCst);
                    if !edit_ptr.is_null() {
                        let edit = HWND(edit_ptr);
                        SetWindowTextW(edit, w!("")).ok();
                        let _ = SetFocus(edit);
                    }
                    let _ = PlaySoundW(w!("SystemExclamation"), None, SND_ALIAS | SND_ASYNC);
                }
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            let edit_ptr = BLOCKING_EDIT_HWND.load(Ordering::SeqCst);
            if !edit_ptr.is_null() {
                let _ = SetFocus(HWND(edit_ptr));
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub unsafe fn register_blocking_class(hinstance: windows::Win32::Foundation::HMODULE) {
    let blocking_class_name = w!("ScreenTimeBlockingClass");
    let blocking_wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(blocking_overlay_proc),
        hInstance: hinstance.into(),
        lpszClassName: blocking_class_name,
        hbrBackground: CreateSolidBrush(COLORREF(COLOR_OVERLAY_BG)),
        hCursor: LoadCursorW(None, IDC_ARROW).ok().unwrap_or_default(),
        ..zeroed()
    };

    if RegisterClassW(&blocking_wnd_class) == 0 {
        panic!("Failed to register blocking overlay window class");
    }

    // Register secondary overlay class (simpler, no controls)
    let secondary_class_name = w!("ScreenTimeSecondaryBlockingClass");
    let secondary_wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(secondary_overlay_proc),
        hInstance: hinstance.into(),
        lpszClassName: secondary_class_name,
        hbrBackground: CreateSolidBrush(COLORREF(COLOR_OVERLAY_BG)),
        hCursor: LoadCursorW(None, IDC_ARROW).ok().unwrap_or_default(),
        ..zeroed()
    };

    if RegisterClassW(&secondary_wnd_class) == 0 {
        panic!("Failed to register secondary blocking overlay window class");
    }
}

/// Window procedure for secondary monitor overlays (simple blank screen)
pub unsafe extern "system" fn secondary_overlay_proc(
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

            // Fill with dark background
            let bg_brush = CreateSolidBrush(COLORREF(COLOR_OVERLAY_BG));
            FillRect(hdc, &rect, bg_brush);
            let _ = DeleteObject(bg_brush);

            // Draw "Screen Locked" text in center
            let font = CreateFontW(
                48, 0, 0, 0,
                FW_BOLD.0 as i32,
                0, 0, 0, 0, 0, 0, 0, 0,
                w!("Segoe UI"),
            );
            let old_font = SelectObject(hdc, font);
            SetTextColor(hdc, COLORREF(COLOR_TEXT_LIGHT));
            SetBkMode(hdc, TRANSPARENT);

            DrawTextW(
                hdc,
                &mut "Screen Locked".encode_utf16().collect::<Vec<_>>(),
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            SelectObject(hdc, old_font);
            let _ = DeleteObject(font);

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_CLOSE => LRESULT(0), // Prevent closing
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Callback for EnumDisplayMonitors to collect monitor information
unsafe extern "system" fn monitor_enum_callback(
    _hmonitor: HMONITOR,
    _hdc: HDC,
    lprect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);

    if let Some(rect) = lprect.as_ref() {
        // Check if this is the primary monitor (origin at 0,0)
        let is_primary = rect.left == 0 && rect.top == 0;

        monitors.push(MonitorInfo {
            rect: *rect,
            is_primary,
        });
    }

    BOOL::from(true) // Continue enumeration
}

/// Enumerate all monitors and return their information
unsafe fn enumerate_monitors() -> Vec<MonitorInfo> {
    let mut monitors: Vec<MonitorInfo> = Vec::new();

    let _ = EnumDisplayMonitors(
        None,
        None,
        Some(monitor_enum_callback),
        LPARAM(&mut monitors as *mut Vec<MonitorInfo> as isize),
    );

    monitors
}

/// Create secondary overlay windows for all non-primary monitors
pub unsafe fn create_secondary_overlays(hinstance: windows::Win32::Foundation::HMODULE) {
    let monitors = enumerate_monitors();
    let class_name = w!("ScreenTimeSecondaryBlockingClass");

    let mut secondary_hwnds = SECONDARY_OVERLAY_HWNDS.lock().unwrap();
    secondary_hwnds.clear();

    for monitor in monitors {
        // Skip the primary monitor (main blocking overlay handles it)
        if monitor.is_primary {
            continue;
        }

        let width = monitor.rect.right - monitor.rect.left;
        let height = monitor.rect.bottom - monitor.rect.top;

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("Screen Time - Locked"),
            WS_POPUP,
            monitor.rect.left,
            monitor.rect.top,
            width,
            height,
            None,
            None,
            hinstance,
            None,
        );

        if let Ok(h) = hwnd {
            secondary_hwnds.push(h.0 as isize);
        }
    }
}

/// Show all secondary monitor overlays
unsafe fn show_secondary_overlays() {
    let secondary_hwnds = SECONDARY_OVERLAY_HWNDS.lock().unwrap();

    for &hwnd_ptr in secondary_hwnds.iter() {
        let hwnd = HWND(hwnd_ptr as *mut std::ffi::c_void);
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0, 0, 0, 0,
            SWP_SHOWWINDOW | SWP_NOMOVE | SWP_NOSIZE,
        ).ok();
        let _ = ShowWindow(hwnd, SW_SHOW);
    }
}

/// Hide all secondary monitor overlays
unsafe fn hide_secondary_overlays() {
    let secondary_hwnds = SECONDARY_OVERLAY_HWNDS.lock().unwrap();

    for &hwnd_ptr in secondary_hwnds.iter() {
        let hwnd = HWND(hwnd_ptr as *mut std::ffi::c_void);
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}
