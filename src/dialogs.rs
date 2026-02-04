//! Dialogs module for Screen Time Manager
//! Contains passcode verification and settings dialog implementations

use std::mem::zeroed;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreateFontW, CreateRoundRectRgn, CreateSolidBrush, DeleteObject,
            DrawTextW, EndPaint, FillRect, InvalidateRect, SelectObject, SetBkMode, SetTextColor,
            SetWindowRgn, DT_CENTER, DT_SINGLELINE, FW_BOLD, FW_NORMAL, PAINTSTRUCT, TRANSPARENT,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::*,
            Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE, VK_RETURN},
            WindowsAndMessaging::*,
        },
    },
};

use crate::constants::*;
use crate::database::{get_passcode, get_setting, set_setting, set_telegram_config, get_telegram_config, WEEKDAY_KEYS, WEEKDAY_NAMES, get_pause_used_today, get_pause_config, get_pause_log_today, is_pause_enabled};
use crate::dpi::scale;

// Control IDs for settings dialog
const ID_SETTINGS_BASE: i32 = 2000;
const ID_SETTINGS_SAVE: i32 = 2100;
const ID_SETTINGS_CANCEL: i32 = 2101;
const ID_CURRENT_PASSCODE: i32 = 2110;
const ID_NEW_PASSCODE: i32 = 2111;
const ID_CONFIRM_PASSCODE: i32 = 2112;

// Settings dialog state
static mut SETTINGS_EDIT_HANDLES: Option<SettingsEditHandles> = None;

struct SettingsEditHandles {
    daily_limits: [HWND; 7],
    warning1_minutes: HWND,
    warning1_message: HWND,
    warning2_minutes: HWND,
    warning2_message: HWND,
    blocking_message: HWND,
    current_passcode: HWND,
    new_passcode: HWND,
    confirm_passcode: HWND,
    // Telegram settings
    telegram_token: HWND,
    telegram_chat_id: HWND,
    telegram_enabled: HWND,
    // Lock screen timeout
    lock_screen_timeout: HWND,
}

/// Verify passcode before allowing sensitive operations
pub unsafe fn verify_passcode_for_quit(parent_hwnd: HWND) -> bool {
    let stored_passcode = match get_passcode() {
        Some(p) => p,
        None => return true,
    };

    let dialog_class = w!("ScreenTimePasscodeDialogNice");
    let hinstance = GetModuleHandleW(None).expect("Failed to get module handle");

    static mut DIALOG_RESULT: Option<bool> = None;
    static mut DIALOG_EDIT_HWND: Option<HWND> = None;
    static mut DIALOG_STORED_CODE: Option<String> = None;
    static mut DIALOG_ERROR: bool = false;

    DIALOG_RESULT = None;
    DIALOG_STORED_CODE = Some(stored_passcode);
    DIALOG_ERROR = false;

    unsafe extern "system" fn dialog_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CREATE => {
                let hinstance = GetModuleHandleW(None).unwrap();

                let edit = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("EDIT"),
                    w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER
                        | WINDOW_STYLE(ES_CENTER as u32 | ES_PASSWORD as u32 | ES_NUMBER as u32),
                    scale(100), scale(95), scale(150), scale(45),
                    hwnd,
                    HMENU(101 as _),
                    hinstance,
                    None,
                ).ok();

                if let Some(e) = edit {
                    DIALOG_EDIT_HWND = Some(e);
                    SendMessageW(e, EM_SETLIMITTEXT, WPARAM(4), LPARAM(0));

                    let hfont = CreateFontW(
                        scale(28), 0, 0, 0,
                        FW_BOLD.0 as i32,
                        0, 0, 0, 0, 0, 0, 0, 0,
                        w!("Segoe UI"),
                    );
                    SendMessageW(e, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    let _ = SetFocus(e);
                }

                // OK Button
                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("BUTTON"),
                    w!("OK"),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    scale(70), scale(200), scale(100), scale(40),
                    hwnd,
                    HMENU(1 as _),
                    hinstance,
                    None,
                );

                // Cancel Button
                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("BUTTON"),
                    w!("Cancel"),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    scale(180), scale(200), scale(100), scale(40),
                    hwnd,
                    HMENU(2 as _),
                    hinstance,
                    None,
                );

                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect: RECT = zeroed();
                GetClientRect(hwnd, &mut rect).ok();

                let bg_brush = CreateSolidBrush(COLORREF(0x00F0F0F0));
                FillRect(hdc, &rect, bg_brush);
                let _ = DeleteObject(bg_brush);

                let title_font = CreateFontW(
                    scale(22), 0, 0, 0,
                    FW_BOLD.0 as i32,
                    0, 0, 0, 0, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                let old_font = SelectObject(hdc, title_font);
                SetTextColor(hdc, COLORREF(0x00333333));
                SetBkMode(hdc, TRANSPARENT);

                let mut title_rect = RECT { left: 0, top: scale(25), right: rect.right, bottom: scale(55) };
                DrawTextW(
                    hdc,
                    &mut "Enter Passcode".encode_utf16().collect::<Vec<_>>(),
                    &mut title_rect,
                    DT_CENTER | DT_SINGLELINE,
                );

                let sub_font = CreateFontW(
                    scale(14), 0, 0, 0,
                    FW_NORMAL.0 as i32,
                    0, 0, 0, 0, 0, 0, 0, 0,
                    w!("Segoe UI"),
                );
                SelectObject(hdc, sub_font);
                SetTextColor(hdc, COLORREF(0x00666666));

                let mut sub_rect = RECT { left: 0, top: scale(55), right: rect.right, bottom: scale(80) };
                DrawTextW(
                    hdc,
                    &mut "Enter 4-digit code to continue".encode_utf16().collect::<Vec<_>>(),
                    &mut sub_rect,
                    DT_CENTER | DT_SINGLELINE,
                );

                if DIALOG_ERROR {
                    SetTextColor(hdc, COLORREF(COLOR_ERROR));
                    let mut err_rect = RECT { left: 0, top: scale(150), right: rect.right, bottom: scale(170) };
                    DrawTextW(
                        hdc,
                        &mut "Incorrect passcode".encode_utf16().collect::<Vec<_>>(),
                        &mut err_rect,
                        DT_CENTER | DT_SINGLELINE,
                    );
                }

                SelectObject(hdc, old_font);
                let _ = DeleteObject(title_font);
                let _ = DeleteObject(sub_font);

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as u16;
                match id {
                    1 => { // OK
                        if let Some(edit_hwnd) = DIALOG_EDIT_HWND {
                            let mut buffer = [0u16; 16];
                            let len = GetWindowTextW(edit_hwnd, &mut buffer);
                            let entered: String = String::from_utf16_lossy(&buffer[..len as usize]);

                            if let Some(ref stored) = DIALOG_STORED_CODE {
                                if entered == *stored {
                                    DIALOG_RESULT = Some(true);
                                    DestroyWindow(hwnd).ok();
                                } else {
                                    DIALOG_ERROR = true;
                                    let _ = InvalidateRect(hwnd, None, true);
                                    SetWindowTextW(edit_hwnd, w!("")).ok();
                                    let _ = SetFocus(edit_hwnd);
                                }
                            }
                        }
                    }
                    2 => { // Cancel
                        DIALOG_RESULT = Some(false);
                        DestroyWindow(hwnd).ok();
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 == VK_RETURN.0 as usize {
                    SendMessageW(hwnd, WM_COMMAND, WPARAM(1), LPARAM(0));
                } else if wparam.0 == VK_ESCAPE.0 as usize {
                    DIALOG_RESULT = Some(false);
                    DestroyWindow(hwnd).ok();
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                DIALOG_RESULT = Some(false);
                DestroyWindow(hwnd).ok();
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    let wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(dialog_proc),
        hInstance: hinstance.into(),
        lpszClassName: dialog_class,
        hbrBackground: CreateSolidBrush(COLORREF(0x00F0F0F0)),
        hCursor: LoadCursorW(None, IDC_ARROW).ok().unwrap_or_default(),
        ..zeroed()
    };
    RegisterClassW(&wnd_class);

    let screen_width = GetSystemMetrics(SM_CXSCREEN);
    let screen_height = GetSystemMetrics(SM_CYSCREEN);
    let dialog_width = scale(350);
    let dialog_height = scale(300);

    let dialog_hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_DLGMODALFRAME,
        dialog_class,
        w!(""),
        WS_POPUP | WS_CAPTION | WS_SYSMENU,
        (screen_width - dialog_width) / 2,
        (screen_height - dialog_height) / 2,
        dialog_width,
        dialog_height,
        parent_hwnd,
        HMENU::default(),
        hinstance,
        None,
    );

    if let Ok(dlg) = dialog_hwnd {
        let rgn = CreateRoundRectRgn(0, 0, dialog_width, dialog_height, scale(10), scale(10));
        SetWindowRgn(dlg, rgn, true);

        let _ = ShowWindow(dlg, SW_SHOW);
        let _ = SetForegroundWindow(dlg);

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    DIALOG_RESULT.unwrap_or(false)
}

/// Show the settings dialog
pub unsafe fn show_settings_dialog(parent_hwnd: HWND) {
    let dialog_class = w!("ScreenTimeSettingsDialog");
    let hinstance = GetModuleHandleW(None).expect("Failed to get module handle");

    static mut SETTINGS_DIALOG_OPEN: bool = false;

    if SETTINGS_DIALOG_OPEN {
        return;
    }
    SETTINGS_DIALOG_OPEN = true;

    unsafe extern "system" fn settings_dialog_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CREATE => {
                let hinstance = GetModuleHandleW(None).unwrap();

                let label_font = CreateFontW(
                    scale(16), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                );
                let title_font = CreateFontW(
                    scale(18), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                );
                let edit_font = CreateFontW(
                    scale(16), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                );

                let mut y_pos = scale(10);

                // ===== Daily Limits Section =====
                let title1 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Daily Time Limits (minutes)"),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(350), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title1 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(22);

                let mut daily_handles: [HWND; 7] = [HWND::default(); 7];

                // Create day controls in pairs (two columns per row)
                // Row 0: Monday (0), Tuesday (1)
                // Row 1: Wednesday (2), Thursday (3)
                // Row 2: Friday (4), Saturday (5)
                // Row 3: Sunday (6) alone
                for row in 0..4 {
                    let i = row * 2; // First column day index

                    // First column
                    let label_text: Vec<u16> = format!("{}:\0", WEEKDAY_NAMES[i]).encode_utf16().collect();
                    let label = CreateWindowExW(
                        WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(label_text.as_ptr()),
                        WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(90), scale(20), hwnd, HMENU::default(), hinstance, None,
                    );
                    if let Ok(h) = label { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }

                    let edit = CreateWindowExW(
                        WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                        WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_NUMBER as u32 | ES_CENTER as u32),
                        scale(120), y_pos, scale(60), scale(22), hwnd, HMENU((ID_SETTINGS_BASE + i as i32) as _), hinstance, None,
                    );
                    if let Ok(h) = edit {
                        SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                        SendMessageW(h, EM_SETLIMITTEXT, WPARAM(4), LPARAM(0));
                        let value = get_setting(WEEKDAY_KEYS[i]).unwrap_or_else(|| "120".to_string());
                        let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                        SetWindowTextW(h, PCWSTR(wide.as_ptr())).ok();
                        daily_handles[i] = h;
                    }

                    // Second column (only if there's a second day in this row)
                    let i2 = i + 1;
                    if i2 < 7 {
                        let label_text2: Vec<u16> = format!("{}:\0", WEEKDAY_NAMES[i2]).encode_utf16().collect();
                        let label2 = CreateWindowExW(
                            WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(label_text2.as_ptr()),
                            WS_CHILD | WS_VISIBLE, scale(210), y_pos + scale(2), scale(90), scale(20), hwnd, HMENU::default(), hinstance, None,
                        );
                        if let Ok(h) = label2 { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }

                        let edit2 = CreateWindowExW(
                            WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                            WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_NUMBER as u32 | ES_CENTER as u32),
                            scale(305), y_pos, scale(60), scale(22), hwnd, HMENU((ID_SETTINGS_BASE + i2 as i32) as _), hinstance, None,
                        );
                        if let Ok(h) = edit2 {
                            SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                            SendMessageW(h, EM_SETLIMITTEXT, WPARAM(4), LPARAM(0));
                            let value = get_setting(WEEKDAY_KEYS[i2]).unwrap_or_else(|| "120".to_string());
                            let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                            SetWindowTextW(h, PCWSTR(wide.as_ptr())).ok();
                            daily_handles[i2] = h;
                        }
                    }

                    y_pos += scale(24);
                }

                // ===== Warning 1 Section =====
                y_pos += scale(4);
                let title2 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("First Warning"),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(350), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title2 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Minutes before:"),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(100), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let w1_min = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_NUMBER as u32 | ES_CENTER as u32),
                    scale(130), y_pos, scale(50), scale(22), hwnd, HMENU((ID_SETTINGS_BASE + 20) as _), hinstance, None,
                );
                let mut w1_min_hwnd = HWND::default();
                if let Ok(h) = w1_min {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    let value = get_setting("warning1_minutes").unwrap_or_else(|| "10".to_string());
                    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                    SetWindowTextW(h, PCWSTR(wide.as_ptr())).ok();
                    w1_min_hwnd = h;
                }
                y_pos += scale(24);

                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Message:"),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(60), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let w1_msg = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER,
                    scale(90), y_pos, scale(275), scale(22), hwnd, HMENU((ID_SETTINGS_BASE + 21) as _), hinstance, None,
                );
                let mut w1_msg_hwnd = HWND::default();
                if let Ok(h) = w1_msg {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    let value = get_setting("warning1_message").unwrap_or_else(|| "10 minutes remaining!".to_string());
                    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                    SetWindowTextW(h, PCWSTR(wide.as_ptr())).ok();
                    w1_msg_hwnd = h;
                }
                y_pos += scale(24);

                // ===== Warning 2 Section =====
                let title3 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Second Warning"),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(350), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title3 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Minutes before:"),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(100), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let w2_min = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_NUMBER as u32 | ES_CENTER as u32),
                    scale(130), y_pos, scale(50), scale(22), hwnd, HMENU((ID_SETTINGS_BASE + 30) as _), hinstance, None,
                );
                let mut w2_min_hwnd = HWND::default();
                if let Ok(h) = w2_min {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    let value = get_setting("warning2_minutes").unwrap_or_else(|| "5".to_string());
                    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                    SetWindowTextW(h, PCWSTR(wide.as_ptr())).ok();
                    w2_min_hwnd = h;
                }
                y_pos += scale(24);

                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Message:"),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(60), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let w2_msg = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER,
                    scale(90), y_pos, scale(275), scale(22), hwnd, HMENU((ID_SETTINGS_BASE + 31) as _), hinstance, None,
                );
                let mut w2_msg_hwnd = HWND::default();
                if let Ok(h) = w2_msg {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    let value = get_setting("warning2_message").unwrap_or_else(|| "5 minutes remaining!".to_string());
                    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                    SetWindowTextW(h, PCWSTR(wide.as_ptr())).ok();
                    w2_msg_hwnd = h;
                }
                y_pos += scale(24);

                // ===== Blocking Message Section =====
                let title4 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Blocking Screen Message"),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(350), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title4 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                let block_msg = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER,
                    scale(25), y_pos, scale(340), scale(22), hwnd, HMENU((ID_SETTINGS_BASE + 40) as _), hinstance, None,
                );
                let mut block_msg_hwnd = HWND::default();
                if let Ok(h) = block_msg {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    let value = get_setting("blocking_message").unwrap_or_else(|| "Your screen time limit has been reached.".to_string());
                    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                    SetWindowTextW(h, PCWSTR(wide.as_ptr())).ok();
                    block_msg_hwnd = h;
                }
                y_pos += scale(24);

                // ===== Change Passcode Section =====
                let title5 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Change Passcode (leave blank to keep)"),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(360), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title5 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Current:"),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(55), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let curr_pass = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_PASSWORD as u32 | ES_NUMBER as u32 | ES_CENTER as u32),
                    scale(80), y_pos, scale(60), scale(22), hwnd, HMENU(ID_CURRENT_PASSCODE as _), hinstance, None,
                );
                let mut curr_pass_hwnd = HWND::default();
                if let Ok(h) = curr_pass {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    SendMessageW(h, EM_SETLIMITTEXT, WPARAM(4), LPARAM(0));
                    curr_pass_hwnd = h;
                }

                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("New:"),
                    WS_CHILD | WS_VISIBLE, scale(155), y_pos + scale(2), scale(35), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let new_pass = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_PASSWORD as u32 | ES_NUMBER as u32 | ES_CENTER as u32),
                    scale(190), y_pos, scale(60), scale(22), hwnd, HMENU(ID_NEW_PASSCODE as _), hinstance, None,
                );
                let mut new_pass_hwnd = HWND::default();
                if let Ok(h) = new_pass {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    SendMessageW(h, EM_SETLIMITTEXT, WPARAM(4), LPARAM(0));
                    new_pass_hwnd = h;
                }

                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Confirm:"),
                    WS_CHILD | WS_VISIBLE, scale(265), y_pos + scale(2), scale(50), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let confirm_pass = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_PASSWORD as u32 | ES_NUMBER as u32 | ES_CENTER as u32),
                    scale(315), y_pos, scale(60), scale(22), hwnd, HMENU(ID_CONFIRM_PASSCODE as _), hinstance, None,
                );
                let mut confirm_pass_hwnd = HWND::default();
                if let Ok(h) = confirm_pass {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    SendMessageW(h, EM_SETLIMITTEXT, WPARAM(4), LPARAM(0));
                    confirm_pass_hwnd = h;
                }
                y_pos += scale(24);

                // ===== Telegram Bot Section =====
                let title6 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Telegram Bot"),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(360), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title6 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                // Enable checkbox
                let telegram_enabled_chk = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), w!("Enable Telegram Bot"),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    scale(25), y_pos, scale(200), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let mut telegram_enabled_hwnd = HWND::default();
                if let Ok(h) = telegram_enabled_chk {
                    SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1));
                    let config = get_telegram_config();
                    if config.enabled {
                        SendMessageW(h, BM_SETCHECK, WPARAM(1), LPARAM(0));
                    }
                    telegram_enabled_hwnd = h;
                }
                y_pos += scale(22);

                // Bot Token
                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Bot Token:"),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(70), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let telegram_token = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_PASSWORD as u32 | ES_AUTOHSCROLL as u32),
                    scale(100), y_pos, scale(265), scale(22), hwnd, HMENU::default(), hinstance, None,
                );
                let mut telegram_token_hwnd = HWND::default();
                if let Ok(h) = telegram_token {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    // Allow long bot tokens (up to 200 chars)
                    SendMessageW(h, EM_SETLIMITTEXT, WPARAM(200), LPARAM(0));
                    let config = get_telegram_config();
                    if let Some(token) = config.bot_token {
                        let wide: Vec<u16> = token.encode_utf16().chain(std::iter::once(0)).collect();
                        SetWindowTextW(h, PCWSTR(wide.as_ptr())).ok();
                    }
                    telegram_token_hwnd = h;
                }
                y_pos += scale(24);

                // Admin Chat ID
                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Chat ID:"),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(70), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let telegram_chat_id = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_NUMBER as u32),
                    scale(100), y_pos, scale(120), scale(22), hwnd, HMENU::default(), hinstance, None,
                );
                let mut telegram_chat_id_hwnd = HWND::default();
                if let Ok(h) = telegram_chat_id {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    let config = get_telegram_config();
                    if let Some(chat_id) = config.admin_chat_id {
                        let value = chat_id.to_string();
                        let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                        SetWindowTextW(h, PCWSTR(wide.as_ptr())).ok();
                    }
                    telegram_chat_id_hwnd = h;
                }
                y_pos += scale(24);

                // ===== Lock Screen Timeout =====
                let title7 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Lock Screen"),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(360), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title7 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                // Lock screen timeout input
                let _ = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Shutdown timeout:"),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(150), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let lock_timeout_edit = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_NUMBER as u32),
                    scale(180), y_pos, scale(60), scale(22), hwnd, HMENU::default(), hinstance, None,
                );
                let mut lock_timeout_hwnd = HWND::default();
                if let Ok(h) = lock_timeout_edit {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    // Load current value (convert seconds to minutes for display)
                    let timeout_secs = crate::database::get_lock_screen_timeout();
                    let timeout_mins = timeout_secs / 60;
                    let value = timeout_mins.to_string();
                    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                    SetWindowTextW(h, PCWSTR(wide.as_ptr())).ok();
                    lock_timeout_hwnd = h;
                }
                y_pos += scale(28);

                // ===== Buttons =====
                let btn_font = CreateFontW(
                    scale(16), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                );

                let save_btn = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), w!("Save"),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    scale(100), y_pos, scale(90), scale(30), hwnd, HMENU(ID_SETTINGS_SAVE as _), hinstance, None,
                );
                if let Ok(h) = save_btn { SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1)); }

                let cancel_btn = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), w!("Cancel"),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    scale(200), y_pos, scale(90), scale(30), hwnd, HMENU(ID_SETTINGS_CANCEL as _), hinstance, None,
                );
                if let Ok(h) = cancel_btn { SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1)); }

                SETTINGS_EDIT_HANDLES = Some(SettingsEditHandles {
                    daily_limits: daily_handles,
                    warning1_minutes: w1_min_hwnd,
                    warning1_message: w1_msg_hwnd,
                    warning2_minutes: w2_min_hwnd,
                    warning2_message: w2_msg_hwnd,
                    blocking_message: block_msg_hwnd,
                    current_passcode: curr_pass_hwnd,
                    new_passcode: new_pass_hwnd,
                    confirm_passcode: confirm_pass_hwnd,
                    telegram_token: telegram_token_hwnd,
                    telegram_chat_id: telegram_chat_id_hwnd,
                    telegram_enabled: telegram_enabled_hwnd,
                    lock_screen_timeout: lock_timeout_hwnd,
                });

                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;

                if id == ID_SETTINGS_SAVE {
                    if let Some(ref handles) = SETTINGS_EDIT_HANDLES {
                        // Handle passcode change first
                        let mut current_pass = String::new();
                        let mut new_pass = String::new();
                        let mut confirm_pass = String::new();

                        if !handles.current_passcode.0.is_null() {
                            let mut buffer = [0u16; 16];
                            let len = GetWindowTextW(handles.current_passcode, &mut buffer);
                            current_pass = String::from_utf16_lossy(&buffer[..len as usize]);
                        }
                        if !handles.new_passcode.0.is_null() {
                            let mut buffer = [0u16; 16];
                            let len = GetWindowTextW(handles.new_passcode, &mut buffer);
                            new_pass = String::from_utf16_lossy(&buffer[..len as usize]);
                        }
                        if !handles.confirm_passcode.0.is_null() {
                            let mut buffer = [0u16; 16];
                            let len = GetWindowTextW(handles.confirm_passcode, &mut buffer);
                            confirm_pass = String::from_utf16_lossy(&buffer[..len as usize]);
                        }

                        // Check if user wants to change passcode
                        if !new_pass.is_empty() || !confirm_pass.is_empty() {
                            // Verify current passcode
                            let stored = get_passcode().unwrap_or_else(|| "0000".to_string());
                            if current_pass != stored {
                                MessageBoxW(hwnd, w!("Current passcode is incorrect!"), w!("Error"), MB_OK | MB_ICONERROR);
                                return LRESULT(0);
                            }

                            // Check new passcode requirements
                            if new_pass.len() != 4 {
                                MessageBoxW(hwnd, w!("New passcode must be exactly 4 digits!"), w!("Error"), MB_OK | MB_ICONERROR);
                                return LRESULT(0);
                            }

                            // Check that new passcodes match
                            if new_pass != confirm_pass {
                                MessageBoxW(hwnd, w!("New passcode and confirmation do not match!"), w!("Error"), MB_OK | MB_ICONERROR);
                                return LRESULT(0);
                            }

                            // Save new passcode
                            set_setting("passcode", &new_pass);
                        }

                        // Save other settings
                        for (i, &edit_hwnd) in handles.daily_limits.iter().enumerate() {
                            if !edit_hwnd.0.is_null() {
                                let mut buffer = [0u16; 16];
                                let len = GetWindowTextW(edit_hwnd, &mut buffer);
                                let value = String::from_utf16_lossy(&buffer[..len as usize]);
                                set_setting(WEEKDAY_KEYS[i], &value);
                            }
                        }

                        if !handles.warning1_minutes.0.is_null() {
                            let mut buffer = [0u16; 16];
                            let len = GetWindowTextW(handles.warning1_minutes, &mut buffer);
                            let value = String::from_utf16_lossy(&buffer[..len as usize]);
                            set_setting("warning1_minutes", &value);
                        }
                        if !handles.warning1_message.0.is_null() {
                            let mut buffer = [0u16; 256];
                            let len = GetWindowTextW(handles.warning1_message, &mut buffer);
                            let value = String::from_utf16_lossy(&buffer[..len as usize]);
                            set_setting("warning1_message", &value);
                        }

                        if !handles.warning2_minutes.0.is_null() {
                            let mut buffer = [0u16; 16];
                            let len = GetWindowTextW(handles.warning2_minutes, &mut buffer);
                            let value = String::from_utf16_lossy(&buffer[..len as usize]);
                            set_setting("warning2_minutes", &value);
                        }
                        if !handles.warning2_message.0.is_null() {
                            let mut buffer = [0u16; 256];
                            let len = GetWindowTextW(handles.warning2_message, &mut buffer);
                            let value = String::from_utf16_lossy(&buffer[..len as usize]);
                            set_setting("warning2_message", &value);
                        }

                        if !handles.blocking_message.0.is_null() {
                            let mut buffer = [0u16; 256];
                            let len = GetWindowTextW(handles.blocking_message, &mut buffer);
                            let value = String::from_utf16_lossy(&buffer[..len as usize]);
                            set_setting("blocking_message", &value);
                        }

                        // Save Telegram settings
                        let mut telegram_token = String::new();
                        let mut telegram_chat_id = String::new();
                        let telegram_enabled;

                        if !handles.telegram_token.0.is_null() {
                            let mut buffer = [0u16; 512];
                            let len = GetWindowTextW(handles.telegram_token, &mut buffer);
                            telegram_token = String::from_utf16_lossy(&buffer[..len as usize]);
                        }

                        if !handles.telegram_chat_id.0.is_null() {
                            let mut buffer = [0u16; 64];
                            let len = GetWindowTextW(handles.telegram_chat_id, &mut buffer);
                            telegram_chat_id = String::from_utf16_lossy(&buffer[..len as usize]);
                        }

                        if !handles.telegram_enabled.0.is_null() {
                            let checked = SendMessageW(handles.telegram_enabled, BM_GETCHECK, WPARAM(0), LPARAM(0));
                            telegram_enabled = checked.0 == 1;
                        } else {
                            telegram_enabled = false;
                        }

                        set_telegram_config(&telegram_token, &telegram_chat_id, telegram_enabled);

                        // Save lock screen timeout (convert minutes to seconds)
                        if !handles.lock_screen_timeout.0.is_null() {
                            let mut buffer = [0u16; 16];
                            let len = GetWindowTextW(handles.lock_screen_timeout, &mut buffer);
                            let value = String::from_utf16_lossy(&buffer[..len as usize]);
                            if let Ok(minutes) = value.parse::<i32>() {
                                let seconds = minutes * 60;
                                set_setting("lock_screen_timeout", &seconds.to_string());
                            }
                        }
                    }

                    MessageBoxW(hwnd, w!("Settings saved successfully!"), w!("Settings"), MB_OK | MB_ICONINFORMATION);
                    DestroyWindow(hwnd).ok();
                } else if id == ID_SETTINGS_CANCEL {
                    DestroyWindow(hwnd).ok();
                }

                LRESULT(0)
            }
            WM_CLOSE => {
                DestroyWindow(hwnd).ok();
                LRESULT(0)
            }
            WM_DESTROY => {
                SETTINGS_EDIT_HANDLES = None;
                SETTINGS_DIALOG_OPEN = false;
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    let wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(settings_dialog_proc),
        hInstance: hinstance.into(),
        lpszClassName: dialog_class,
        hbrBackground: CreateSolidBrush(COLORREF(0x00F5F5F5)),
        hCursor: LoadCursorW(None, IDC_ARROW).ok().unwrap_or_default(),
        ..zeroed()
    };
    RegisterClassW(&wnd_class);

    let screen_width = GetSystemMetrics(SM_CXSCREEN);
    let screen_height = GetSystemMetrics(SM_CYSCREEN);
    let dialog_width = scale(400);
    let dialog_height = scale(580);

    let dialog_hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_DLGMODALFRAME,
        dialog_class,
        w!("Screen Time Settings"),
        WS_POPUP | WS_CAPTION | WS_SYSMENU,
        (screen_width - dialog_width) / 2,
        (screen_height - dialog_height) / 2,
        dialog_width,
        dialog_height,
        parent_hwnd,
        HMENU::default(),
        hinstance,
        None,
    );

    if let Ok(dlg) = dialog_hwnd {
        let rgn = CreateRoundRectRgn(0, 0, dialog_width, dialog_height, scale(10), scale(10));
        SetWindowRgn(dlg, rgn, true);

        let _ = ShowWindow(dlg, SW_SHOW);
        let _ = SetForegroundWindow(dlg);

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    SETTINGS_DIALOG_OPEN = false;
}

/// Show today's stats dialog
pub unsafe fn show_stats_dialog(parent_hwnd: HWND) {
    let dialog_class = w!("ScreenTimeStatsDialog");
    let hinstance = GetModuleHandleW(None).expect("Failed to get module handle");

    static mut STATS_DIALOG_OPEN: bool = false;

    if STATS_DIALOG_OPEN {
        return;
    }
    STATS_DIALOG_OPEN = true;

    unsafe extern "system" fn stats_dialog_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        use crate::blocking::REMAINING_SECONDS;
        use crate::database::{get_current_weekday, get_daily_limit, save_remaining_time};
        use crate::mini_overlay::update_mini_overlay;
        use std::sync::atomic::Ordering;

        const ID_RESET_TIMER: i32 = 3001;
        const ID_CLOSE: i32 = 3002;

        match msg {
            WM_CREATE => {
                let hinstance = GetModuleHandleW(None).unwrap();

                let btn_font = CreateFontW(
                    scale(16), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                );

                // Reset Timer button
                let reset_btn = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), w!("Reset Timer"),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    scale(50), scale(310), scale(120), scale(35), hwnd, HMENU(ID_RESET_TIMER as _), hinstance, None,
                );
                if let Ok(h) = reset_btn { SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1)); }

                // Close button
                let close_btn = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), w!("Close"),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    scale(190), scale(310), scale(100), scale(35), hwnd, HMENU(ID_CLOSE as _), hinstance, None,
                );
                if let Ok(h) = close_btn { SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1)); }

                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect: RECT = zeroed();
                GetClientRect(hwnd, &mut rect).ok();

                let bg_brush = CreateSolidBrush(COLORREF(0x00F5F5F5));
                FillRect(hdc, &rect, bg_brush);
                let _ = DeleteObject(bg_brush);

                // Get stats
                let weekday = get_current_weekday();
                let daily_limit_minutes = get_daily_limit(weekday);
                let daily_limit_seconds = (daily_limit_minutes * 60) as i32;
                let remaining_seconds = REMAINING_SECONDS.load(Ordering::SeqCst);
                let used_seconds = if remaining_seconds >= 0 {
                    daily_limit_seconds - remaining_seconds
                } else {
                    0
                };

                // Get pause stats
                let pause_enabled = is_pause_enabled();
                let pause_config = get_pause_config();
                let pause_used_seconds = get_pause_used_today();
                let pause_budget_seconds = (pause_config.daily_budget_minutes * 60) as i32;
                let pause_remaining_seconds = (pause_budget_seconds - pause_used_seconds).max(0);
                let pause_log = get_pause_log_today();

                // Format time helper
                fn format_duration(seconds: i32) -> String {
                    if seconds < 0 {
                        return String::from("--");
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

                // Get weekday name
                let weekday_names = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
                let weekday_name = weekday_names.get(weekday as usize).unwrap_or(&"Unknown");

                // Title font (DPI scaled)
                let title_font = CreateFontW(
                    scale(22), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                );
                let section_font = CreateFontW(
                    scale(16), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                );
                let label_font = CreateFontW(
                    scale(15), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                );
                let value_font = CreateFontW(
                    scale(16), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                );
                let small_font = CreateFontW(
                    scale(13), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                );

                let old_font = SelectObject(hdc, title_font);
                SetTextColor(hdc, COLORREF(0x00333333));
                SetBkMode(hdc, TRANSPARENT);

                // Title
                let mut title_rect = RECT { left: 0, top: scale(15), right: rect.right, bottom: scale(42) };
                DrawTextW(
                    hdc,
                    &mut "Today's Statistics".encode_utf16().collect::<Vec<_>>(),
                    &mut title_rect,
                    DT_CENTER | DT_SINGLELINE,
                );

                // Stats
                let mut y = scale(50);
                let left_margin = scale(25);
                let value_x = scale(160);

                // Day
                SelectObject(hdc, label_font);
                SetTextColor(hdc, COLORREF(0x00666666));
                let mut label_rect = RECT { left: left_margin, top: y, right: value_x, bottom: y + scale(22) };
                DrawTextW(hdc, &mut "Day:".encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

                SelectObject(hdc, value_font);
                SetTextColor(hdc, COLORREF(0x00333333));
                let day_str = format!("{}", weekday_name);
                let mut value_rect = RECT { left: value_x, top: y, right: rect.right - scale(15), bottom: y + scale(22) };
                DrawTextW(hdc, &mut day_str.encode_utf16().collect::<Vec<_>>(), &mut value_rect, DT_SINGLELINE);
                y += scale(24);

                // Daily limit
                SelectObject(hdc, label_font);
                SetTextColor(hdc, COLORREF(0x00666666));
                let mut label_rect = RECT { left: left_margin, top: y, right: value_x, bottom: y + scale(22) };
                DrawTextW(hdc, &mut "Daily Limit:".encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

                SelectObject(hdc, value_font);
                SetTextColor(hdc, COLORREF(0x00333333));
                let limit_str = format!("{} min", daily_limit_minutes);
                let mut value_rect = RECT { left: value_x, top: y, right: rect.right - scale(15), bottom: y + scale(22) };
                DrawTextW(hdc, &mut limit_str.encode_utf16().collect::<Vec<_>>(), &mut value_rect, DT_SINGLELINE);
                y += scale(24);

                // Time used
                SelectObject(hdc, label_font);
                SetTextColor(hdc, COLORREF(0x00666666));
                let mut label_rect = RECT { left: left_margin, top: y, right: value_x, bottom: y + scale(22) };
                DrawTextW(hdc, &mut "Time Used:".encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

                SelectObject(hdc, value_font);
                SetTextColor(hdc, COLORREF(0x00333333));
                let used_str = format_duration(used_seconds.max(0));
                let mut value_rect = RECT { left: value_x, top: y, right: rect.right - scale(15), bottom: y + scale(22) };
                DrawTextW(hdc, &mut used_str.encode_utf16().collect::<Vec<_>>(), &mut value_rect, DT_SINGLELINE);
                y += scale(24);

                // Time remaining
                SelectObject(hdc, label_font);
                SetTextColor(hdc, COLORREF(0x00666666));
                let mut label_rect = RECT { left: left_margin, top: y, right: value_x, bottom: y + scale(22) };
                DrawTextW(hdc, &mut "Time Remaining:".encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

                SelectObject(hdc, value_font);
                // Color based on remaining time
                if remaining_seconds <= 60 {
                    SetTextColor(hdc, COLORREF(COLOR_ERROR));
                } else if remaining_seconds <= 300 {
                    SetTextColor(hdc, COLORREF(COLOR_ACCENT));
                } else {
                    SetTextColor(hdc, COLORREF(0x00008800)); // Green
                }
                let remaining_str = format_duration(remaining_seconds);
                let mut value_rect = RECT { left: value_x, top: y, right: rect.right - scale(15), bottom: y + scale(22) };
                DrawTextW(hdc, &mut remaining_str.encode_utf16().collect::<Vec<_>>(), &mut value_rect, DT_SINGLELINE);
                y += scale(32);

                // ===== Pause Section =====
                SelectObject(hdc, section_font);
                SetTextColor(hdc, COLORREF(0x00333333));
                let mut section_rect = RECT { left: left_margin, top: y, right: rect.right - scale(15), bottom: y + scale(20) };
                DrawTextW(hdc, &mut "Pause Mode".encode_utf16().collect::<Vec<_>>(), &mut section_rect, DT_SINGLELINE);
                y += scale(22);

                if pause_enabled {
                    // Pause budget used
                    SelectObject(hdc, label_font);
                    SetTextColor(hdc, COLORREF(0x00666666));
                    let mut label_rect = RECT { left: left_margin, top: y, right: value_x, bottom: y + scale(22) };
                    DrawTextW(hdc, &mut "Pause Used:".encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

                    SelectObject(hdc, value_font);
                    SetTextColor(hdc, COLORREF(0x00333333));
                    let pause_used_str = format!("{} / {} min", pause_used_seconds / 60, pause_config.daily_budget_minutes);
                    let mut value_rect = RECT { left: value_x, top: y, right: rect.right - scale(15), bottom: y + scale(22) };
                    DrawTextW(hdc, &mut pause_used_str.encode_utf16().collect::<Vec<_>>(), &mut value_rect, DT_SINGLELINE);
                    y += scale(24);

                    // Pause remaining
                    SelectObject(hdc, label_font);
                    SetTextColor(hdc, COLORREF(0x00666666));
                    let mut label_rect = RECT { left: left_margin, top: y, right: value_x, bottom: y + scale(22) };
                    DrawTextW(hdc, &mut "Pause Remaining:".encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

                    SelectObject(hdc, value_font);
                    if pause_remaining_seconds <= 0 {
                        SetTextColor(hdc, COLORREF(COLOR_ERROR));
                    } else if pause_remaining_seconds <= 300 {
                        SetTextColor(hdc, COLORREF(COLOR_ACCENT));
                    } else {
                        SetTextColor(hdc, COLORREF(0x00008800)); // Green
                    }
                    let pause_remaining_str = format_duration(pause_remaining_seconds);
                    let mut value_rect = RECT { left: value_x, top: y, right: rect.right - scale(15), bottom: y + scale(22) };
                    DrawTextW(hdc, &mut pause_remaining_str.encode_utf16().collect::<Vec<_>>(), &mut value_rect, DT_SINGLELINE);
                    y += scale(24);

                    // Pause count
                    SelectObject(hdc, label_font);
                    SetTextColor(hdc, COLORREF(0x00666666));
                    let mut label_rect = RECT { left: left_margin, top: y, right: value_x, bottom: y + scale(22) };
                    DrawTextW(hdc, &mut "Pauses Today:".encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

                    SelectObject(hdc, value_font);
                    SetTextColor(hdc, COLORREF(0x00333333));
                    let pause_count_str = format!("{}", pause_log.len());
                    let mut value_rect = RECT { left: value_x, top: y, right: rect.right - scale(15), bottom: y + scale(22) };
                    DrawTextW(hdc, &mut pause_count_str.encode_utf16().collect::<Vec<_>>(), &mut value_rect, DT_SINGLELINE);
                    y += scale(24);

                    // Pause log (if any)
                    if !pause_log.is_empty() {
                        SelectObject(hdc, small_font);
                        SetTextColor(hdc, COLORREF(0x00888888));
                        let log_str = format!("Log: {}", pause_log.join(", "));
                        let mut log_rect = RECT { left: left_margin, top: y, right: rect.right - scale(15), bottom: y + scale(18) };
                        DrawTextW(hdc, &mut log_str.encode_utf16().collect::<Vec<_>>(), &mut log_rect, DT_SINGLELINE);
                    }
                } else {
                    SelectObject(hdc, label_font);
                    SetTextColor(hdc, COLORREF(0x00888888));
                    let mut disabled_rect = RECT { left: left_margin, top: y, right: rect.right - scale(15), bottom: y + scale(22) };
                    DrawTextW(hdc, &mut "Pause feature is disabled".encode_utf16().collect::<Vec<_>>(), &mut disabled_rect, DT_SINGLELINE);
                }

                SelectObject(hdc, old_font);
                let _ = DeleteObject(title_font);
                let _ = DeleteObject(section_font);
                let _ = DeleteObject(label_font);
                let _ = DeleteObject(value_font);
                let _ = DeleteObject(small_font);

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;

                if id == ID_RESET_TIMER {
                    // Reset to daily limit
                    let weekday = get_current_weekday();
                    let daily_limit_minutes = get_daily_limit(weekday);
                    let daily_limit_seconds = (daily_limit_minutes * 60) as i32;

                    REMAINING_SECONDS.store(daily_limit_seconds, Ordering::SeqCst);
                    save_remaining_time(daily_limit_seconds);
                    update_mini_overlay();

                    MessageBoxW(hwnd, w!("Timer has been reset to the daily limit."), w!("Timer Reset"), MB_OK | MB_ICONINFORMATION);
                    let _ = InvalidateRect(hwnd, None, true);
                } else if id == ID_CLOSE {
                    DestroyWindow(hwnd).ok();
                }

                LRESULT(0)
            }
            WM_CLOSE => {
                DestroyWindow(hwnd).ok();
                LRESULT(0)
            }
            WM_DESTROY => {
                STATS_DIALOG_OPEN = false;
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    let wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(stats_dialog_proc),
        hInstance: hinstance.into(),
        lpszClassName: dialog_class,
        hbrBackground: CreateSolidBrush(COLORREF(0x00F5F5F5)),
        hCursor: LoadCursorW(None, IDC_ARROW).ok().unwrap_or_default(),
        ..zeroed()
    };
    RegisterClassW(&wnd_class);

    let screen_width = GetSystemMetrics(SM_CXSCREEN);
    let screen_height = GetSystemMetrics(SM_CYSCREEN);
    let dialog_width = scale(340);
    let dialog_height = scale(390);

    let dialog_hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_DLGMODALFRAME,
        dialog_class,
        w!("Today's Stats"),
        WS_POPUP | WS_CAPTION | WS_SYSMENU,
        (screen_width - dialog_width) / 2,
        (screen_height - dialog_height) / 2,
        dialog_width,
        dialog_height,
        parent_hwnd,
        HMENU::default(),
        hinstance,
        None,
    );

    if let Ok(dlg) = dialog_hwnd {
        let rgn = CreateRoundRectRgn(0, 0, dialog_width, dialog_height, scale(10), scale(10));
        SetWindowRgn(dlg, rgn, true);

        let _ = ShowWindow(dlg, SW_SHOW);
        let _ = SetForegroundWindow(dlg);

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    STATS_DIALOG_OPEN = false;
}
