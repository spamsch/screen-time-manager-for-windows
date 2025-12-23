//! Mini overlay module
//! Small, always-visible display showing remaining time

use std::mem::zeroed;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicPtr, Ordering};
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
use crate::database;

/// Global state for mini overlay window
pub static MINI_OVERLAY_HWND: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static MINI_OVERLAY_VISIBLE: AtomicBool = AtomicBool::new(false);

// Pause state tracking
pub static IS_PAUSED: AtomicBool = AtomicBool::new(false);
pub static PAUSE_START_TIMESTAMP: AtomicI64 = AtomicI64::new(0);
pub static CURRENT_PAUSE_DURATION: AtomicI32 = AtomicI32::new(0);
pub static SESSION_ACTIVE_SECONDS: AtomicI32 = AtomicI32::new(0);

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

// ============================================================================
// Pause Mode Functions
// ============================================================================

/// Check if timer is currently paused
pub fn is_paused() -> bool {
    IS_PAUSED.load(Ordering::SeqCst)
}

/// Reason why pause is not available
#[derive(Debug, Clone)]
pub enum PauseBlockedReason {
    Disabled,
    BudgetExhausted,
    CooldownActive { seconds_remaining: i32 },
    MinActiveTimeNotMet { seconds_remaining: i32 },
    TimeTooLow,
}

/// Check if pause is currently available and return reason if not
pub fn can_pause() -> Result<(), PauseBlockedReason> {
    // Already paused - can always unpause
    if IS_PAUSED.load(Ordering::SeqCst) {
        return Ok(());
    }

    // Check if pause feature is enabled
    if !database::is_pause_enabled() {
        return Err(PauseBlockedReason::Disabled);
    }

    let config = database::get_pause_config();

    // Check if remaining time is too low (< 1 minute)
    let remaining = REMAINING_SECONDS.load(Ordering::SeqCst);
    if remaining < 60 {
        return Err(PauseBlockedReason::TimeTooLow);
    }

    // Check daily budget
    let pause_used = database::get_pause_used_today();
    let budget_seconds = (config.daily_budget_minutes * 60) as i32;
    if pause_used >= budget_seconds {
        return Err(PauseBlockedReason::BudgetExhausted);
    }

    // Check cooldown
    let last_pause_end = database::get_last_pause_end();
    let current_time = database::get_current_timestamp();
    let cooldown_seconds = (config.cooldown_minutes * 60) as i64;
    let time_since_last_pause = current_time - last_pause_end;

    if last_pause_end > 0 && time_since_last_pause < cooldown_seconds {
        let remaining_cooldown = (cooldown_seconds - time_since_last_pause) as i32;
        return Err(PauseBlockedReason::CooldownActive {
            seconds_remaining: remaining_cooldown,
        });
    }

    // Check minimum active time
    let session_active = SESSION_ACTIVE_SECONDS.load(Ordering::SeqCst);
    let min_active_seconds = (config.min_active_time_minutes * 60) as i32;

    if session_active < min_active_seconds {
        let remaining_active = min_active_seconds - session_active;
        return Err(PauseBlockedReason::MinActiveTimeNotMet {
            seconds_remaining: remaining_active,
        });
    }

    Ok(())
}

/// Get remaining pause budget in seconds
pub fn get_remaining_pause_budget() -> i32 {
    let config = database::get_pause_config();
    let budget_seconds = (config.daily_budget_minutes * 60) as i32;
    let used = database::get_pause_used_today();
    (budget_seconds - used).max(0)
}

/// Get maximum pause duration for current pause (considering budget and config)
pub fn get_max_pause_duration() -> i32 {
    let config = database::get_pause_config();
    let max_single = (config.max_duration_minutes * 60) as i32;
    let remaining_budget = get_remaining_pause_budget();
    max_single.min(remaining_budget)
}

/// Toggle pause state - returns true if now paused, false if resumed
pub fn toggle_pause() -> Result<bool, PauseBlockedReason> {
    if IS_PAUSED.load(Ordering::SeqCst) {
        // Currently paused - resume
        resume_timer();
        Ok(false)
    } else {
        // Not paused - try to pause
        can_pause()?;
        pause_timer();
        Ok(true)
    }
}

/// Pause the timer
fn pause_timer() {
    let timestamp = database::get_current_timestamp();
    PAUSE_START_TIMESTAMP.store(timestamp, Ordering::SeqCst);
    CURRENT_PAUSE_DURATION.store(0, Ordering::SeqCst);
    IS_PAUSED.store(true, Ordering::SeqCst);

    // Update display immediately
    unsafe {
        let hwnd = HWND(MINI_OVERLAY_HWND.load(Ordering::SeqCst));
        if !hwnd.0.is_null() {
            let _ = InvalidateRect(hwnd, None, true);
        }
    }
}

/// Resume the timer (end pause)
fn resume_timer() {
    let pause_duration = CURRENT_PAUSE_DURATION.load(Ordering::SeqCst);

    // Update total pause used today
    let total_used = database::get_pause_used_today() + pause_duration;
    database::save_pause_used_today(total_used);

    // Log the pause event
    database::log_pause_event(pause_duration);

    // Save last pause end timestamp
    let timestamp = database::get_current_timestamp();
    database::save_last_pause_end(timestamp);

    // Reset pause state
    IS_PAUSED.store(false, Ordering::SeqCst);
    PAUSE_START_TIMESTAMP.store(0, Ordering::SeqCst);
    CURRENT_PAUSE_DURATION.store(0, Ordering::SeqCst);

    // Update display immediately
    unsafe {
        let hwnd = HWND(MINI_OVERLAY_HWND.load(Ordering::SeqCst));
        if !hwnd.0.is_null() {
            let _ = InvalidateRect(hwnd, None, true);
        }
    }
}

/// Force resume (called when max duration reached)
fn force_resume() {
    resume_timer();
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

            let paused = IS_PAUSED.load(Ordering::SeqCst);

            // Background color changes when paused
            let bg_color = if paused { 0x00332200 } else { 0x00222222 }; // Brownish when paused
            let bg_brush = CreateSolidBrush(COLORREF(bg_color));
            FillRect(hdc, &rect, bg_brush);
            let _ = DeleteObject(bg_brush);

            // Get remaining time and pause info
            let remaining = REMAINING_SECONDS.load(Ordering::SeqCst);

            let (display_text, color) = if paused {
                // Show pause indicator and remaining pause time
                let pause_duration = CURRENT_PAUSE_DURATION.load(Ordering::SeqCst);
                let max_duration = get_max_pause_duration();
                let pause_remaining = max_duration - pause_duration;

                // Format: "⏸ 0:45" (pause symbol + remaining pause time)
                let pause_time_str = format_time_compact(pause_remaining);
                (format!("II {}", pause_time_str), 0x0066CCFF_u32) // Cyan/light blue for paused
            } else {
                // Normal display
                let time_str = format_time_compact(remaining);
                let color = get_time_color(remaining);
                (time_str, color)
            };

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
                let paused = IS_PAUSED.load(Ordering::SeqCst);

                if paused {
                    // Timer is paused - increment pause duration instead
                    let duration = CURRENT_PAUSE_DURATION.fetch_add(1, Ordering::SeqCst) + 1;
                    let max_duration = get_max_pause_duration();

                    // Check if max pause duration reached
                    if duration >= max_duration {
                        // Auto-resume
                        force_resume();
                    }
                } else {
                    // Timer is running normally
                    let current = REMAINING_SECONDS.load(Ordering::SeqCst);
                    if current > 0 {
                        let new_time = current - 1;
                        REMAINING_SECONDS.store(new_time, Ordering::SeqCst);

                        // Increment session active time
                        SESSION_ACTIVE_SECONDS.fetch_add(1, Ordering::SeqCst);

                        // Save to database periodically (every 30 seconds)
                        if new_time % 30 == 0 {
                            database::save_remaining_time(new_time);
                            // Also save session active time
                            let active = SESSION_ACTIVE_SECONDS.load(Ordering::SeqCst);
                            database::save_session_active_time(active);
                        }

                        // Check for warning 1 (e.g., 10 minutes remaining)
                        let (warn1_mins, warn1_msg) = database::get_warning_config(1);
                        if new_time == (warn1_mins * 60) as i32 {
                            crate::overlay::show_overlay(&warn1_msg, 10);
                        }

                        // Check for warning 2 (e.g., 5 minutes remaining)
                        let (warn2_mins, warn2_msg) = database::get_warning_config(2);
                        if new_time == (warn2_mins * 60) as i32 {
                            crate::overlay::show_overlay(&warn2_msg, 10);
                        }

                        // Trigger blocking overlay when time reaches 0
                        if new_time == 0 {
                            let msg = database::get_blocking_message();
                            crate::blocking::show_blocking_overlay(&msg);
                        }
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
