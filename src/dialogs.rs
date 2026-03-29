//! Dialogs module for Screen Time Manager
//! Contains passcode verification and settings dialog implementations

use std::mem::zeroed;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreateFontW, CreatePen, CreateRoundRectRgn, CreateSolidBrush, DeleteObject,
            DrawTextW, Ellipse, EndPaint, FillRect, InvalidateRect, LineTo, MoveToEx, SelectObject,
            SetBkMode, SetTextColor, SetWindowRgn, DT_CENTER, DT_SINGLELINE, DT_VCENTER, FW_BOLD,
            FW_NORMAL, HDC, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::*,
            Input::KeyboardAndMouse::{EnableWindow, SetFocus, VK_ESCAPE, VK_RETURN},
            WindowsAndMessaging::*,
        },
    },
};

use crate::constants::*;
use crate::database::{get_passcode, get_setting, set_setting, set_telegram_config, get_telegram_config, WEEKDAY_KEYS, get_pause_used_today, get_pause_config, get_pause_log_today, is_pause_enabled, is_idle_enabled, get_idle_timeout_minutes};
use crate::dpi::scale;
use crate::i18n::{self, Language};

// Control IDs for settings dialog
const ID_SETTINGS_BASE: i32 = 2000;
const ID_SETTINGS_SAVE: i32 = 2100;
const ID_SETTINGS_CANCEL: i32 = 2101;
const ID_CURRENT_PASSCODE: i32 = 2110;
const ID_NEW_PASSCODE: i32 = 2111;
const ID_CONFIRM_PASSCODE: i32 = 2112;
const ID_LANGUAGE_COMBO: i32 = 2120;
const ID_TELEGRAM_WIZARD: i32 = 2130;

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
    // Idle detection settings
    idle_enabled: HWND,
    idle_timeout_minutes: HWND,
    // Language setting
    language: HWND,
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
                    scale(100), scale(100), scale(150), scale(36),
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
                        0, 0, 0, 0, 0, 0, 5, 0,
                        w!("Segoe UI"),
                    );
                    SendMessageW(e, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    let _ = SetFocus(e);
                }

                // Button font
                let btn_font = CreateFontW(
                    scale(14), 0, 0, 0,
                    FW_NORMAL.0 as i32,
                    0, 0, 0, 0, 0, 0, 5, 0,
                    w!("Segoe UI"),
                );

                // OK Button
                let ok_btn_text = i18n::wide("button.ok");
                let ok_btn = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("BUTTON"),
                    PCWSTR(ok_btn_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    scale(70), scale(200), scale(100), scale(40),
                    hwnd,
                    HMENU(1 as _),
                    hinstance,
                    None,
                );
                if let Ok(h) = ok_btn { SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1)); }

                // Cancel Button
                let cancel_btn_text = i18n::wide("button.cancel");
                let cancel_btn = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("BUTTON"),
                    PCWSTR(cancel_btn_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    scale(180), scale(200), scale(100), scale(40),
                    hwnd,
                    HMENU(2 as _),
                    hinstance,
                    None,
                );
                if let Ok(h) = cancel_btn { SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1)); }

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
                    scale(20), 0, 0, 0,
                    FW_BOLD.0 as i32,
                    0, 0, 0, 0, 0, 0, 5, 0,
                    w!("Segoe UI"),
                );
                let old_font = SelectObject(hdc, title_font);
                SetTextColor(hdc, COLORREF(0x00333333));
                SetBkMode(hdc, TRANSPARENT);

                let mut title_rect = RECT { left: 0, top: scale(25), right: rect.right, bottom: scale(55) };
                let title_text: Vec<u16> = i18n::t("window.passcode").encode_utf16().collect();
                DrawTextW(
                    hdc,
                    &mut title_text.clone(),
                    &mut title_rect,
                    DT_CENTER | DT_SINGLELINE,
                );

                let sub_font = CreateFontW(
                    scale(13), 0, 0, 0,
                    FW_NORMAL.0 as i32,
                    0, 0, 0, 0, 0, 0, 5, 0,
                    w!("Segoe UI"),
                );
                SelectObject(hdc, sub_font);
                SetTextColor(hdc, COLORREF(0x00666666));

                let mut sub_rect = RECT { left: 0, top: scale(55), right: rect.right, bottom: scale(80) };
                let sub_text: Vec<u16> = i18n::t("passcode.subtitle").encode_utf16().collect();
                DrawTextW(
                    hdc,
                    &mut sub_text.clone(),
                    &mut sub_rect,
                    DT_CENTER | DT_SINGLELINE,
                );

                if DIALOG_ERROR {
                    SetTextColor(hdc, COLORREF(COLOR_ERROR));
                    let mut err_rect = RECT { left: 0, top: scale(150), right: rect.right, bottom: scale(170) };
                    let err_text: Vec<u16> = i18n::t("passcode.incorrect").encode_utf16().collect();
                    DrawTextW(
                        hdc,
                        &mut err_text.clone(),
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

                // Font quality: 5 = CLEARTYPE_QUALITY for crisp rendering
                let label_font = CreateFontW(
                    scale(14), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"),
                );
                let title_font = CreateFontW(
                    scale(16), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"),
                );
                let edit_font = CreateFontW(
                    scale(14), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"),
                );

                let mut y_pos = scale(10);

                // ===== Language Section =====
                let lang_label = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), w!("Language / Sprache:"),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos + scale(2), scale(140), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = lang_label { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }

                let lang_combo = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("COMBOBOX"), w!(""),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    scale(160), y_pos, scale(100), scale(200), hwnd, HMENU(ID_LANGUAGE_COMBO as _), hinstance, None,
                );
                let mut lang_combo_hwnd = HWND::default();
                if let Ok(h) = lang_combo {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    // Add language options
                    for lang in Language::all() {
                        let name: Vec<u16> = lang.name().encode_utf16().chain(std::iter::once(0)).collect();
                        SendMessageW(h, CB_ADDSTRING, WPARAM(0), LPARAM(name.as_ptr() as isize));
                    }
                    // Select current language
                    let current_lang = i18n::current();
                    let index = match current_lang {
                        Language::English => 0,
                        Language::German => 1,
                    };
                    SendMessageW(h, CB_SETCURSEL, WPARAM(index), LPARAM(0));
                    lang_combo_hwnd = h;
                }
                y_pos += scale(28);

                // ===== Daily Limits Section =====
                let title1_text = i18n::wide("settings.daily_limits");
                let title1 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(title1_text.as_ptr()),
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
                    let label_text: Vec<u16> = format!("{}:\0", i18n::weekday(i)).encode_utf16().collect();
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
                        let label_text2: Vec<u16> = format!("{}:\0", i18n::weekday(i2)).encode_utf16().collect();
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
                y_pos += scale(10);
                let title2_text = i18n::wide("settings.warning1");
                let title2 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(title2_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(350), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title2 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                let min_label1_text = i18n::wide("settings.minutes_before");
                let min_label1 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(min_label1_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(100), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = min_label1 { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
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

                let msg_label1_text = i18n::wide("settings.message");
                let msg_label1 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(msg_label1_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(60), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = msg_label1 { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
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
                y_pos += scale(10);
                let title3_text = i18n::wide("settings.warning2");
                let title3 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(title3_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(350), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title3 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                let min_label2_text = i18n::wide("settings.minutes_before");
                let min_label2 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(min_label2_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(100), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = min_label2 { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
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

                let msg_label2_text = i18n::wide("settings.message");
                let msg_label2 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(msg_label2_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(60), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = msg_label2 { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
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
                y_pos += scale(10);
                let title4_text = i18n::wide("settings.blocking_message");
                let title4 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(title4_text.as_ptr()),
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
                y_pos += scale(10);
                let title5_text = i18n::wide("settings.passcode");
                let title5 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(title5_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(360), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title5 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                let curr_label_text = i18n::wide("settings.current");
                let curr_label = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(curr_label_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(55), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = curr_label { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
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

                let new_label_text = i18n::wide("settings.new");
                let new_label = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(new_label_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(155), y_pos + scale(2), scale(35), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = new_label { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
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

                let confirm_label_text = i18n::wide("settings.confirm");
                let confirm_label = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(confirm_label_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(265), y_pos + scale(2), scale(50), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = confirm_label { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
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
                y_pos += scale(10);
                let title6_text = i18n::wide("settings.telegram");
                let title6 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(title6_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(360), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title6 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                // Enable checkbox
                let telegram_chk_text = i18n::wide("settings.enable_telegram");
                let telegram_enabled_chk = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), PCWSTR(telegram_chk_text.as_ptr()),
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
                let bot_token_label_text = i18n::wide("settings.bot_token");
                let bot_token_label = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(bot_token_label_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(70), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = bot_token_label { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
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
                let chat_id_label_text = i18n::wide("settings.chat_id");
                let chat_id_label = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(chat_id_label_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(70), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = chat_id_label { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
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

                // Setup Wizard button (to the right of chat_id)
                let wizard_btn_text = i18n::wide("settings.setup_wizard");
                let wizard_btn = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), PCWSTR(wizard_btn_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    scale(230), y_pos, scale(135), scale(22), hwnd, HMENU(ID_TELEGRAM_WIZARD as _), hinstance, None,
                );
                if let Ok(h) = wizard_btn { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
                y_pos += scale(24);

                // ===== Lock Screen Timeout =====
                y_pos += scale(10);
                let title7_text = i18n::wide("settings.lock_screen");
                let title7 = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(title7_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(360), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title7 { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                // Lock screen timeout input
                let shutdown_label_text = i18n::wide("settings.shutdown_timeout");
                let shutdown_label = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(shutdown_label_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(150), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = shutdown_label { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
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
                y_pos += scale(24);

                // ===== Idle Detection Section =====
                y_pos += scale(10);
                let title_idle_text = i18n::wide("settings.idle");
                let title_idle = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(title_idle_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(15), y_pos, scale(360), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = title_idle { SendMessageW(h, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1)); }
                y_pos += scale(20);

                // Enable checkbox
                let idle_chk_text = i18n::wide("settings.auto_pause_idle");
                let idle_enabled_chk = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), PCWSTR(idle_chk_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    scale(25), y_pos, scale(200), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                let mut idle_enabled_hwnd = HWND::default();
                if let Ok(h) = idle_enabled_chk {
                    SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1));
                    if is_idle_enabled() {
                        SendMessageW(h, BM_SETCHECK, WPARAM(1), LPARAM(0));
                    }
                    idle_enabled_hwnd = h;
                }
                y_pos += scale(22);

                // Timeout minutes
                let idle_timeout_label_text = i18n::wide("settings.idle_timeout");
                let idle_timeout_label = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(idle_timeout_label_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE, scale(25), y_pos + scale(2), scale(150), scale(20), hwnd, HMENU::default(), hinstance, None,
                );
                if let Ok(h) = idle_timeout_label { SendMessageW(h, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1)); }
                let idle_timeout_edit = CreateWindowExW(
                    WINDOW_EX_STYLE(0x200), w!("EDIT"), w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_NUMBER as u32 | ES_CENTER as u32),
                    scale(180), y_pos, scale(50), scale(22), hwnd, HMENU::default(), hinstance, None,
                );
                let mut idle_timeout_hwnd = HWND::default();
                if let Ok(h) = idle_timeout_edit {
                    SendMessageW(h, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));
                    SendMessageW(h, EM_SETLIMITTEXT, WPARAM(3), LPARAM(0));
                    let value = get_idle_timeout_minutes().to_string();
                    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                    SetWindowTextW(h, PCWSTR(wide.as_ptr())).ok();
                    idle_timeout_hwnd = h;
                }
                y_pos += scale(28);

                // ===== Buttons =====
                let btn_font = CreateFontW(
                    scale(14), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"),
                );

                let save_btn_text = i18n::wide("button.save");
                let save_btn = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), PCWSTR(save_btn_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    scale(100), y_pos, scale(90), scale(30), hwnd, HMENU(ID_SETTINGS_SAVE as _), hinstance, None,
                );
                if let Ok(h) = save_btn { SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1)); }

                let cancel_btn_text = i18n::wide("button.cancel");
                let cancel_btn = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), PCWSTR(cancel_btn_text.as_ptr()),
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
                    idle_enabled: idle_enabled_hwnd,
                    idle_timeout_minutes: idle_timeout_hwnd,
                    language: lang_combo_hwnd,
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
                                let msg = i18n::wide("settings.error.current_incorrect");
                                let title = i18n::wide("settings.error");
                                MessageBoxW(hwnd, PCWSTR(msg.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONERROR);
                                return LRESULT(0);
                            }

                            // Check new passcode requirements
                            if new_pass.len() != 4 {
                                let msg = i18n::wide("settings.error.passcode_length");
                                let title = i18n::wide("settings.error");
                                MessageBoxW(hwnd, PCWSTR(msg.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONERROR);
                                return LRESULT(0);
                            }

                            // Check that new passcodes match
                            if new_pass != confirm_pass {
                                let msg = i18n::wide("settings.error.passcode_mismatch");
                                let title = i18n::wide("settings.error");
                                MessageBoxW(hwnd, PCWSTR(msg.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONERROR);
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

                        // If today's limit was lowered, cap remaining time to the new limit
                        {
                            use crate::blocking::REMAINING_SECONDS;
                            use crate::database::{get_current_weekday, get_daily_limit, save_remaining_time};
                            use crate::mini_overlay::update_mini_overlay;
                            use std::sync::atomic::Ordering;

                            let weekday = get_current_weekday();
                            let new_limit_seconds = (get_daily_limit(weekday) * 60) as i32;
                            let remaining = REMAINING_SECONDS.load(Ordering::SeqCst);

                            if remaining > new_limit_seconds {
                                REMAINING_SECONDS.store(new_limit_seconds, Ordering::SeqCst);
                                save_remaining_time(new_limit_seconds);
                                update_mini_overlay();
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

                        // Save idle detection settings
                        if !handles.idle_enabled.0.is_null() {
                            let checked = SendMessageW(handles.idle_enabled, BM_GETCHECK, WPARAM(0), LPARAM(0));
                            set_setting("idle_enabled", if checked.0 == 1 { "1" } else { "0" });
                        }
                        if !handles.idle_timeout_minutes.0.is_null() {
                            let mut buffer = [0u16; 16];
                            let len = GetWindowTextW(handles.idle_timeout_minutes, &mut buffer);
                            let value = String::from_utf16_lossy(&buffer[..len as usize]);
                            if let Ok(mins) = value.parse::<u32>() {
                                let clamped = mins.max(1);
                                set_setting("idle_timeout_minutes", &clamped.to_string());
                            }
                        }

                        // Save language setting
                        if !handles.language.0.is_null() {
                            let sel = SendMessageW(handles.language, CB_GETCURSEL, WPARAM(0), LPARAM(0));
                            let lang = match sel.0 {
                                1 => Language::German,
                                _ => Language::English,
                            };
                            i18n::set_language(lang);
                        }
                    }

                    let msg = i18n::wide("settings.success.saved");
                    let title = i18n::wide("settings.success");
                    MessageBoxW(hwnd, PCWSTR(msg.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONINFORMATION);
                    DestroyWindow(hwnd).ok();
                } else if id == ID_SETTINGS_CANCEL {
                    DestroyWindow(hwnd).ok();
                } else if id == ID_TELEGRAM_WIZARD {
                    // Open Telegram setup wizard
                    show_telegram_wizard(hwnd);
                    // Refresh dialog to show new values if wizard completed
                    let _ = InvalidateRect(hwnd, None, true);
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
    let dialog_height = scale(770);

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
                    scale(14), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"),
                );

                // Reset Timer button
                let reset_btn_text = i18n::wide("button.reset_timer");
                let reset_btn = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), PCWSTR(reset_btn_text.as_ptr()),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    scale(50), scale(310), scale(120), scale(35), hwnd, HMENU(ID_RESET_TIMER as _), hinstance, None,
                );
                if let Ok(h) = reset_btn { SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1)); }

                // Close button
                let close_btn_text = i18n::wide("button.close");
                let close_btn = CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("BUTTON"), PCWSTR(close_btn_text.as_ptr()),
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

                // Title font (DPI scaled, ClearType quality = 5)
                let title_font = CreateFontW(
                    scale(20), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"),
                );
                let section_font = CreateFontW(
                    scale(14), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"),
                );
                let label_font = CreateFontW(
                    scale(13), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"),
                );
                let value_font = CreateFontW(
                    scale(14), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"),
                );
                let small_font = CreateFontW(
                    scale(12), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"),
                );

                let old_font = SelectObject(hdc, title_font);
                SetTextColor(hdc, COLORREF(0x00333333));
                SetBkMode(hdc, TRANSPARENT);

                // Title
                let mut title_rect = RECT { left: 0, top: scale(15), right: rect.right, bottom: scale(42) };
                let stats_title: Vec<u16> = i18n::t("stats.title").encode_utf16().collect();
                DrawTextW(
                    hdc,
                    &mut stats_title.clone(),
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
                let day_label: Vec<u16> = i18n::t("stats.day").encode_utf16().collect();
                DrawTextW(hdc, &mut day_label.clone(), &mut label_rect, DT_SINGLELINE);

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
                DrawTextW(hdc, &mut i18n::t("stats.daily_limit").encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

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
                DrawTextW(hdc, &mut i18n::t("stats.time_used").encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

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
                DrawTextW(hdc, &mut i18n::t("stats.time_remaining").encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

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
                DrawTextW(hdc, &mut i18n::t("stats.pause_mode").encode_utf16().collect::<Vec<_>>(), &mut section_rect, DT_SINGLELINE);
                y += scale(22);

                if pause_enabled {
                    // Pause budget used
                    SelectObject(hdc, label_font);
                    SetTextColor(hdc, COLORREF(0x00666666));
                    let mut label_rect = RECT { left: left_margin, top: y, right: value_x, bottom: y + scale(22) };
                    DrawTextW(hdc, &mut i18n::t("stats.pause_used").encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

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
                    DrawTextW(hdc, &mut i18n::t("stats.pause_remaining").encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

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
                    DrawTextW(hdc, &mut i18n::t("stats.pauses_today").encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_SINGLELINE);

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
                        let log_str = format!("{} {}", i18n::t("stats.log"), pause_log.join(", "));
                        let mut log_rect = RECT { left: left_margin, top: y, right: rect.right - scale(15), bottom: y + scale(18) };
                        DrawTextW(hdc, &mut log_str.encode_utf16().collect::<Vec<_>>(), &mut log_rect, DT_SINGLELINE);
                    }
                } else {
                    SelectObject(hdc, label_font);
                    SetTextColor(hdc, COLORREF(0x00888888));
                    let mut disabled_rect = RECT { left: left_margin, top: y, right: rect.right - scale(15), bottom: y + scale(22) };
                    DrawTextW(hdc, &mut i18n::t("stats.pause_disabled").encode_utf16().collect::<Vec<_>>(), &mut disabled_rect, DT_SINGLELINE);
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

                    let msg = i18n::wide("stats.timer_reset");
                    let title = i18n::wide("stats.timer_reset_title");
                    MessageBoxW(hwnd, PCWSTR(msg.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONINFORMATION);
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

    let window_title = i18n::wide("window.stats");
    let dialog_hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_DLGMODALFRAME,
        dialog_class,
        PCWSTR(window_title.as_ptr()),
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

// ============================================================================
// Telegram Setup Wizard
// ============================================================================

// Wizard control IDs
const ID_WIZARD_NEXT: i32 = 3001;
const ID_WIZARD_BACK: i32 = 3002;
const ID_WIZARD_CANCEL: i32 = 3003;
const ID_WIZARD_TOKEN_EDIT: i32 = 3004;

// Wizard state
static mut WIZARD_STEP: i32 = 1;
static mut WIZARD_TOKEN: Option<String> = None;
static mut WIZARD_CHAT_ID: Option<i64> = None;
static mut WIZARD_POLLING: bool = false;
static mut WIZARD_HWND: Option<HWND> = None;
const WIZARD_TOTAL_STEPS: i32 = 5;
const TIMER_POLL_TELEGRAM: usize = 100;

/// Show the Telegram setup wizard
pub unsafe fn show_telegram_wizard(parent_hwnd: HWND) {
    // Reset wizard state
    WIZARD_STEP = 1;
    WIZARD_TOKEN = None;
    WIZARD_CHAT_ID = None;
    WIZARD_POLLING = false;

    let hinstance = GetModuleHandleW(None).unwrap();
    let wizard_class = w!("ScreenTimeTelegramWizard");

    // Register wizard window class
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wizard_proc),
        hInstance: hinstance.into(),
        lpszClassName: wizard_class,
        hbrBackground: CreateSolidBrush(COLORREF(0x00FFFFFF)),
        hCursor: LoadCursorW(None, IDC_ARROW).ok().unwrap_or_default(),
        ..zeroed()
    };
    RegisterClassW(&wc);

    let screen_width = GetSystemMetrics(SM_CXSCREEN);
    let screen_height = GetSystemMetrics(SM_CYSCREEN);
    let dialog_width = scale(480);
    let dialog_height = scale(520);

    let window_title = i18n::wide("wizard.title");
    let wizard_hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_DLGMODALFRAME,
        wizard_class,
        PCWSTR(window_title.as_ptr()),
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

    if let Ok(hwnd) = wizard_hwnd {
        WIZARD_HWND = Some(hwnd);
        let rgn = CreateRoundRectRgn(0, 0, dialog_width, dialog_height, scale(12), scale(12));
        SetWindowRgn(hwnd, rgn, true);

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    WIZARD_HWND = None;
}

/// Wizard window procedure
unsafe extern "system" fn wizard_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            create_wizard_buttons(hwnd);
            LRESULT(0)
        }
        WM_PAINT => {
            paint_wizard(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            handle_wizard_command(hwnd, id);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TIMER_POLL_TELEGRAM {
                poll_telegram_for_chatid(hwnd);
            }
            LRESULT(0)
        }
        WM_CLOSE | WM_DESTROY => {
            KillTimer(hwnd, TIMER_POLL_TELEGRAM).ok();
            WIZARD_POLLING = false;
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Create wizard navigation buttons
unsafe fn create_wizard_buttons(hwnd: HWND) {
    let hinstance = GetModuleHandleW(None).unwrap();

    let mut rect: RECT = zeroed();
    GetClientRect(hwnd, &mut rect).ok();
    let width = rect.right;
    let height = rect.bottom;

    let btn_width = scale(100);
    let btn_height = scale(36);
    let btn_y = height - scale(60);
    let margin = scale(20);

    // Button font
    let btn_font = CreateFontW(
        scale(15), 0, 0, 0,
        FW_NORMAL.0 as i32,
        0, 0, 0, 0, 0, 0, 5, 0,
        w!("Segoe UI"),
    );

    // Cancel button (left)
    let cancel_text = i18n::wide("wizard.cancel");
    let cancel_btn = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        PCWSTR(cancel_text.as_ptr()),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
        margin,
        btn_y,
        btn_width,
        btn_height,
        hwnd,
        HMENU(ID_WIZARD_CANCEL as _),
        hinstance,
        None,
    );
    if let Ok(h) = cancel_btn {
        SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1));
    }

    // Back button
    let back_text = i18n::wide("wizard.back");
    let back_btn = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        PCWSTR(back_text.as_ptr()),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
        width - margin - btn_width * 2 - scale(10),
        btn_y,
        btn_width,
        btn_height,
        hwnd,
        HMENU(ID_WIZARD_BACK as _),
        hinstance,
        None,
    );
    if let Ok(h) = back_btn {
        SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1));
    }

    // Next/Finish button
    let next_text = i18n::wide("wizard.next");
    let next_btn = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        PCWSTR(next_text.as_ptr()),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
        width - margin - btn_width,
        btn_y,
        btn_width,
        btn_height,
        hwnd,
        HMENU(ID_WIZARD_NEXT as _),
        hinstance,
        None,
    );
    if let Ok(h) = next_btn {
        SendMessageW(h, WM_SETFONT, WPARAM(btn_font.0 as usize), LPARAM(1));
    }
}

/// Paint the wizard content based on current step
unsafe fn paint_wizard(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);

    let mut rect: RECT = zeroed();
    GetClientRect(hwnd, &mut rect).ok();
    let width = rect.right;

    // Background
    let bg_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
    FillRect(hdc, &rect, bg_brush);
    let _ = DeleteObject(bg_brush);

    // Header area with gradient-like color
    let header_rect = RECT { left: 0, top: 0, right: width, bottom: scale(100) };
    let header_brush = CreateSolidBrush(COLORREF(0x00F5E6D3)); // Warm beige
    FillRect(hdc, &header_rect, header_brush);
    let _ = DeleteObject(header_brush);

    SetBkMode(hdc, TRANSPARENT);

    // Draw step indicator circles
    draw_step_indicators(hdc, width, WIZARD_STEP);

    // Content area
    let content_top = scale(110);
    let content_rect = RECT {
        left: scale(30),
        top: content_top,
        right: width - scale(30),
        bottom: rect.bottom - scale(80),
    };

    match WIZARD_STEP {
        1 => paint_step_welcome(hdc, &content_rect),
        2 => paint_step_botfather(hdc, &content_rect),
        3 => paint_step_token(hdc, hwnd, &content_rect),
        4 => paint_step_connect(hdc, &content_rect),
        5 => paint_step_success(hdc, &content_rect),
        _ => {}
    }

    // Update button states
    update_wizard_buttons(hwnd);

    let _ = EndPaint(hwnd, &ps);
}

/// Draw step indicator circles at the top
unsafe fn draw_step_indicators(hdc: HDC, width: i32, current_step: i32) {
    let center_y = scale(50);
    let circle_radius = scale(16);
    let spacing = scale(70);
    let start_x = (width - (spacing * 4)) / 2;

    // Font for step numbers
    let num_font = CreateFontW(
        scale(14), 0, 0, 0,
        FW_BOLD.0 as i32,
        0, 0, 0, 0, 0, 0, 5, 0,
        w!("Segoe UI"),
    );
    let old_font = SelectObject(hdc, num_font);

    for i in 1..=5 {
        let cx = start_x + (i - 1) * spacing;

        // Draw connecting line (except for first circle)
        if i > 1 {
            let line_color = if i <= current_step {
                COLORREF(0x004CAF50) // Green
            } else {
                COLORREF(0x00CCCCCC) // Gray
            };
            let pen = CreatePen(PS_SOLID, scale(2), line_color);
            let old_pen = SelectObject(hdc, pen);
            let _ = MoveToEx(hdc, cx - spacing + circle_radius, center_y, None);
            let _ = LineTo(hdc, cx - circle_radius, center_y);
            SelectObject(hdc, old_pen);
            let _ = DeleteObject(pen);
        }

        // Circle color based on state
        let (fill_color, text_color) = if i < current_step {
            (COLORREF(0x004CAF50), COLORREF(0x00FFFFFF)) // Completed: green
        } else if i == current_step {
            (COLORREF(0x002196F3), COLORREF(0x00FFFFFF)) // Current: blue
        } else {
            (COLORREF(0x00EEEEEE), COLORREF(0x00666666)) // Future: gray
        };

        // Draw circle
        let brush = CreateSolidBrush(fill_color);
        let pen = CreatePen(PS_SOLID, 1, fill_color);
        let old_brush = SelectObject(hdc, brush);
        let old_pen = SelectObject(hdc, pen);
        let _ = Ellipse(hdc, cx - circle_radius, center_y - circle_radius,
                cx + circle_radius, center_y + circle_radius);
        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        let _ = DeleteObject(brush);
        let _ = DeleteObject(pen);

        // Draw number or checkmark
        SetTextColor(hdc, text_color);
        let text = if i < current_step {
            "✓".to_string()
        } else {
            i.to_string()
        };
        let mut text_rect = RECT {
            left: cx - circle_radius,
            top: center_y - circle_radius,
            right: cx + circle_radius,
            bottom: center_y + circle_radius,
        };
        DrawTextW(hdc, &mut text.encode_utf16().collect::<Vec<_>>(), &mut text_rect,
                  DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }

    SelectObject(hdc, old_font);
    let _ = DeleteObject(num_font);
}

/// Step 1: Welcome screen
unsafe fn paint_step_welcome(hdc: HDC, rect: &RECT) {
    let title_font = CreateFontW(scale(28), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));
    let desc_font = CreateFontW(scale(16), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));
    let icon_font = CreateFontW(scale(48), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI Emoji"));

    let mut y = rect.top;

    // Phone icon
    SelectObject(hdc, icon_font);
    SetTextColor(hdc, COLORREF(0x002196F3));
    let mut icon_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(60) };
    DrawTextW(hdc, &mut "📱".encode_utf16().collect::<Vec<_>>(), &mut icon_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(70);

    // Title
    SelectObject(hdc, title_font);
    SetTextColor(hdc, COLORREF(0x00333333));
    let mut title_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(35) };
    DrawTextW(hdc, &mut i18n::t("wizard.welcome.title").encode_utf16().collect::<Vec<_>>(), &mut title_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(45);

    // Description
    SelectObject(hdc, desc_font);
    SetTextColor(hdc, COLORREF(0x00666666));
    let mut desc_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(25) };
    DrawTextW(hdc, &mut i18n::t("wizard.welcome.desc1").encode_utf16().collect::<Vec<_>>(), &mut desc_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(40);

    // Features with icons
    let features = [
        ("✓", i18n::t("wizard.welcome.feature1")),
        ("✓", i18n::t("wizard.welcome.feature2")),
        ("✓", i18n::t("wizard.welcome.feature3")),
        ("✓", i18n::t("wizard.welcome.feature4")),
    ];

    SetTextColor(hdc, COLORREF(0x004CAF50));
    for (icon, text) in features.iter() {
        let feature_text = format!("  {}  {}", icon, text);
        let mut feature_rect = RECT { left: rect.left + scale(40), top: y, right: rect.right, bottom: y + scale(28) };
        DrawTextW(hdc, &mut feature_text.encode_utf16().collect::<Vec<_>>(), &mut feature_rect, DT_SINGLELINE);
        y += scale(30);
    }

    y += scale(20);

    // Ready message
    SetTextColor(hdc, COLORREF(0x00333333));
    let mut ready_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(25) };
    DrawTextW(hdc, &mut i18n::t("wizard.welcome.ready").encode_utf16().collect::<Vec<_>>(), &mut ready_rect, DT_CENTER | DT_SINGLELINE);

    let _ = DeleteObject(title_font);
    let _ = DeleteObject(desc_font);
    let _ = DeleteObject(icon_font);
}

/// Step 2: BotFather instructions
unsafe fn paint_step_botfather(hdc: HDC, rect: &RECT) {
    let title_font = CreateFontW(scale(24), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));
    let step_font = CreateFontW(scale(15), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));
    let icon_font = CreateFontW(scale(40), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI Emoji"));
    let hint_font = CreateFontW(scale(13), 0, 0, 0, FW_NORMAL.0 as i32, 1, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));

    let mut y = rect.top;

    // Robot icon
    SelectObject(hdc, icon_font);
    SetTextColor(hdc, COLORREF(0x002196F3));
    let mut icon_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(50) };
    DrawTextW(hdc, &mut "🤖".encode_utf16().collect::<Vec<_>>(), &mut icon_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(55);

    // Title
    SelectObject(hdc, title_font);
    SetTextColor(hdc, COLORREF(0x00333333));
    let mut title_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(30) };
    DrawTextW(hdc, &mut i18n::t("wizard.bot.title").encode_utf16().collect::<Vec<_>>(), &mut title_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(45);

    // Steps
    SelectObject(hdc, step_font);
    let steps = [
        i18n::t("wizard.bot.step1"),
        i18n::t("wizard.bot.step2"),
        i18n::t("wizard.bot.step3"),
        i18n::t("wizard.bot.step4"),
        i18n::t("wizard.bot.step5"),
        i18n::t("wizard.bot.step6"),
    ];

    for step in steps.iter() {
        SetTextColor(hdc, COLORREF(0x00444444));
        let mut step_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(24) };
        DrawTextW(hdc, &mut step.encode_utf16().collect::<Vec<_>>(), &mut step_rect, DT_SINGLELINE);
        y += scale(28);
    }

    y += scale(15);

    // Hint box
    let hint_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(40) };
    let hint_brush = CreateSolidBrush(COLORREF(0x00FFF3E0)); // Light orange
    FillRect(hdc, &hint_rect, hint_brush);
    let _ = DeleteObject(hint_brush);

    SelectObject(hdc, hint_font);
    SetTextColor(hdc, COLORREF(0x00E65100));
    let mut hint_text_rect = RECT { left: rect.left + scale(10), top: y + scale(10), right: rect.right - scale(10), bottom: y + scale(35) };
    DrawTextW(hdc, &mut i18n::t("wizard.bot.hint").encode_utf16().collect::<Vec<_>>(), &mut hint_text_rect, DT_CENTER | DT_SINGLELINE);

    let _ = DeleteObject(title_font);
    let _ = DeleteObject(step_font);
    let _ = DeleteObject(icon_font);
    let _ = DeleteObject(hint_font);
}

/// Step 3: Token entry
unsafe fn paint_step_token(hdc: HDC, hwnd: HWND, rect: &RECT) {
    let title_font = CreateFontW(scale(24), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));
    let label_font = CreateFontW(scale(15), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));
    let icon_font = CreateFontW(scale(40), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI Emoji"));

    let mut y = rect.top;

    // Key icon
    SelectObject(hdc, icon_font);
    SetTextColor(hdc, COLORREF(0x00FF9800));
    let mut icon_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(50) };
    DrawTextW(hdc, &mut "🔑".encode_utf16().collect::<Vec<_>>(), &mut icon_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(55);

    // Title
    SelectObject(hdc, title_font);
    SetTextColor(hdc, COLORREF(0x00333333));
    let mut title_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(30) };
    DrawTextW(hdc, &mut i18n::t("wizard.token.title").encode_utf16().collect::<Vec<_>>(), &mut title_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(50);

    // Label
    SelectObject(hdc, label_font);
    SetTextColor(hdc, COLORREF(0x00666666));
    let mut label_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(22) };
    DrawTextW(hdc, &mut i18n::t("wizard.token.label").encode_utf16().collect::<Vec<_>>(), &mut label_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(30);

    // Create/update edit control for token
    let edit_hwnd = GetDlgItem(hwnd, ID_WIZARD_TOKEN_EDIT).unwrap_or_default();
    if edit_hwnd.0.is_null() {
        let hinstance = GetModuleHandleW(None).unwrap();
        let edit_width = scale(350);
        let edit_x = (rect.right - rect.left - edit_width) / 2 + rect.left;

        let _ = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_CENTER as u32 | ES_AUTOHSCROLL as u32),
            edit_x,
            y,
            edit_width,
            scale(35),
            hwnd,
            HMENU(ID_WIZARD_TOKEN_EDIT as _),
            hinstance,
            None,
        );

        let edit_font = CreateFontW(scale(14), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Consolas"));
        let new_edit = GetDlgItem(hwnd, ID_WIZARD_TOKEN_EDIT).unwrap_or_default();
        if !new_edit.0.is_null() {
            SendMessageW(new_edit, WM_SETFONT, WPARAM(edit_font.0 as usize), LPARAM(1));

            // Set placeholder text if we have a saved token
            if let Some(ref token) = WIZARD_TOKEN {
                let wide: Vec<u16> = token.encode_utf16().chain(std::iter::once(0)).collect();
                SetWindowTextW(new_edit, PCWSTR(wide.as_ptr())).ok();
            }
        }
    }

    y += scale(55);

    // Validation status
    let wizard_token_ref = std::ptr::addr_of!(WIZARD_TOKEN);
    let token_valid = (*wizard_token_ref).as_ref().map(|t| is_valid_token_format(t)).unwrap_or(false);
    if (*wizard_token_ref).is_some() {
        let (status_text, status_color) = if token_valid {
            (format!("✓ {}", i18n::t("wizard.token.valid")), COLORREF(0x004CAF50))
        } else {
            (format!("✗ {}", i18n::t("wizard.token.invalid")), COLORREF(0x00F44336))
        };
        SetTextColor(hdc, status_color);
        let mut status_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(22) };
        DrawTextW(hdc, &mut status_text.encode_utf16().collect::<Vec<_>>(), &mut status_rect, DT_CENTER | DT_SINGLELINE);
    }

    let _ = DeleteObject(title_font);
    let _ = DeleteObject(label_font);
    let _ = DeleteObject(icon_font);
}

/// Step 4: Connect to bot
unsafe fn paint_step_connect(hdc: HDC, rect: &RECT) {
    let title_font = CreateFontW(scale(24), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));
    let step_font = CreateFontW(scale(15), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));
    let icon_font = CreateFontW(scale(40), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI Emoji"));
    let status_font = CreateFontW(scale(18), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));

    let mut y = rect.top;

    // Link icon
    SelectObject(hdc, icon_font);
    SetTextColor(hdc, COLORREF(0x009C27B0));
    let mut icon_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(50) };
    DrawTextW(hdc, &mut "🔗".encode_utf16().collect::<Vec<_>>(), &mut icon_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(55);

    // Title
    SelectObject(hdc, title_font);
    SetTextColor(hdc, COLORREF(0x00333333));
    let mut title_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(30) };
    DrawTextW(hdc, &mut i18n::t("wizard.connect.title").encode_utf16().collect::<Vec<_>>(), &mut title_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(50);

    // Steps
    SelectObject(hdc, step_font);
    let steps = [
        i18n::t("wizard.connect.step1"),
        i18n::t("wizard.connect.step2"),
        i18n::t("wizard.connect.step3"),
    ];

    for step in steps.iter() {
        SetTextColor(hdc, COLORREF(0x00444444));
        let mut step_rect = RECT { left: rect.left + scale(30), top: y, right: rect.right, bottom: y + scale(24) };
        DrawTextW(hdc, &mut step.encode_utf16().collect::<Vec<_>>(), &mut step_rect, DT_SINGLELINE);
        y += scale(32);
    }

    y += scale(30);

    // Status
    SelectObject(hdc, status_font);
    if let Some(chat_id) = WIZARD_CHAT_ID {
        // Connected!
        SetTextColor(hdc, COLORREF(0x004CAF50));
        let detected = format!("✓ {}", i18n::t("wizard.connect.detected"));
        let mut status_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(25) };
        DrawTextW(hdc, &mut detected.encode_utf16().collect::<Vec<_>>(), &mut status_rect, DT_CENTER | DT_SINGLELINE);
        y += scale(30);

        SetTextColor(hdc, COLORREF(0x00666666));
        let chatid_text = format!("{} {}", i18n::t("wizard.connect.chatid"), chat_id);
        let mut chatid_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(22) };
        DrawTextW(hdc, &mut chatid_text.encode_utf16().collect::<Vec<_>>(), &mut chatid_rect, DT_CENTER | DT_SINGLELINE);
    } else {
        // Waiting...
        SetTextColor(hdc, COLORREF(0x00FF9800));
        let waiting_anim = match (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() % 4) as usize {
            0 => "⏳",
            1 => "⌛",
            2 => "⏳",
            _ => "⌛",
        };
        let waiting_text = format!("{} {}", waiting_anim, i18n::t("wizard.connect.waiting"));
        let mut status_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(25) };
        DrawTextW(hdc, &mut waiting_text.encode_utf16().collect::<Vec<_>>(), &mut status_rect, DT_CENTER | DT_SINGLELINE);
    }

    let _ = DeleteObject(title_font);
    let _ = DeleteObject(step_font);
    let _ = DeleteObject(icon_font);
    let _ = DeleteObject(status_font);
}

/// Step 5: Success
unsafe fn paint_step_success(hdc: HDC, rect: &RECT) {
    let title_font = CreateFontW(scale(28), 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));
    let desc_font = CreateFontW(scale(15), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI"));
    let cmd_font = CreateFontW(scale(13), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Consolas"));
    let icon_font = CreateFontW(scale(56), 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 5, 0, w!("Segoe UI Emoji"));

    let mut y = rect.top;

    // Checkmark icon
    SelectObject(hdc, icon_font);
    SetTextColor(hdc, COLORREF(0x004CAF50));
    let mut icon_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(65) };
    DrawTextW(hdc, &mut "✅".encode_utf16().collect::<Vec<_>>(), &mut icon_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(75);

    // Title
    SelectObject(hdc, title_font);
    SetTextColor(hdc, COLORREF(0x004CAF50));
    let mut title_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(35) };
    DrawTextW(hdc, &mut i18n::t("wizard.success.title").encode_utf16().collect::<Vec<_>>(), &mut title_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(45);

    // Description
    SelectObject(hdc, desc_font);
    SetTextColor(hdc, COLORREF(0x00666666));
    let mut desc_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(22) };
    DrawTextW(hdc, &mut i18n::t("wizard.success.desc").encode_utf16().collect::<Vec<_>>(), &mut desc_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(25);

    let mut test_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(22) };
    DrawTextW(hdc, &mut i18n::t("wizard.success.test").encode_utf16().collect::<Vec<_>>(), &mut test_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(40);

    // Commands section
    SetTextColor(hdc, COLORREF(0x00333333));
    let mut cmd_title_rect = RECT { left: rect.left, top: y, right: rect.right, bottom: y + scale(22) };
    DrawTextW(hdc, &mut i18n::t("wizard.success.commands").encode_utf16().collect::<Vec<_>>(), &mut cmd_title_rect, DT_CENTER | DT_SINGLELINE);
    y += scale(30);

    // Command examples with background
    SelectObject(hdc, cmd_font);
    let commands = [
        i18n::t("wizard.success.cmd1"),
        i18n::t("wizard.success.cmd2"),
        i18n::t("wizard.success.cmd3"),
        i18n::t("wizard.success.cmd4"),
    ];

    let cmd_bg = CreateSolidBrush(COLORREF(0x00F5F5F5));
    for cmd in commands.iter() {
        let cmd_rect = RECT { left: rect.left + scale(20), top: y, right: rect.right - scale(20), bottom: y + scale(22) };
        FillRect(hdc, &cmd_rect, cmd_bg);
        SetTextColor(hdc, COLORREF(0x00333333));
        let mut text_rect = RECT { left: rect.left + scale(30), top: y + scale(2), right: rect.right - scale(30), bottom: y + scale(20) };
        DrawTextW(hdc, &mut cmd.encode_utf16().collect::<Vec<_>>(), &mut text_rect, DT_SINGLELINE);
        y += scale(26);
    }
    let _ = DeleteObject(cmd_bg);

    let _ = DeleteObject(title_font);
    let _ = DeleteObject(desc_font);
    let _ = DeleteObject(cmd_font);
    let _ = DeleteObject(icon_font);
}

/// Check if token format is valid (basic check)
fn is_valid_token_format(token: &str) -> bool {
    // Telegram bot tokens are like: 123456789:ABCdefGHI-jklMNO_pqr
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 2 {
        return false;
    }
    // First part should be numeric (bot ID)
    if parts[0].parse::<i64>().is_err() {
        return false;
    }
    // Second part should be alphanumeric with some special chars
    parts[1].len() >= 30 && parts[1].chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Update wizard button states based on current step
unsafe fn update_wizard_buttons(hwnd: HWND) {
    let back_btn = GetDlgItem(hwnd, ID_WIZARD_BACK).unwrap_or_default();
    let next_btn = GetDlgItem(hwnd, ID_WIZARD_NEXT).unwrap_or_default();

    // Back button: disabled on step 1
    if !back_btn.0.is_null() {
        let _ = EnableWindow(back_btn, WIZARD_STEP > 1);
    }

    // Next button text and state
    if !next_btn.0.is_null() {
        let next_text = if WIZARD_STEP == WIZARD_TOTAL_STEPS {
            i18n::wide("wizard.finish")
        } else {
            i18n::wide("wizard.next")
        };
        SetWindowTextW(next_btn, PCWSTR(next_text.as_ptr())).ok();

        // Disable next on step 3 if token invalid, step 4 if not connected
        let wizard_token_ref = std::ptr::addr_of!(WIZARD_TOKEN);
        let wizard_chat_id_ref = std::ptr::addr_of!(WIZARD_CHAT_ID);
        let can_proceed = match WIZARD_STEP {
            3 => (*wizard_token_ref).as_ref().map(|t| is_valid_token_format(t)).unwrap_or(false),
            4 => (*wizard_chat_id_ref).is_some(),
            _ => true,
        };
        let _ = EnableWindow(next_btn, can_proceed);
    }
}

/// Handle wizard command (button clicks)
unsafe fn handle_wizard_command(hwnd: HWND, id: i32) {
    match id {
        ID_WIZARD_CANCEL => {
            DestroyWindow(hwnd).ok();
        }
        ID_WIZARD_BACK => {
            if WIZARD_STEP > 1 {
                // Stop polling if we're leaving step 4
                if WIZARD_STEP == 4 {
                    KillTimer(hwnd, TIMER_POLL_TELEGRAM).ok();
                    WIZARD_POLLING = false;
                }
                WIZARD_STEP -= 1;
                // Destroy token edit if going back from step 3
                if WIZARD_STEP == 2 {
                    let edit = GetDlgItem(hwnd, ID_WIZARD_TOKEN_EDIT).unwrap_or_default();
                    if !edit.0.is_null() {
                        DestroyWindow(edit).ok();
                    }
                }
                let _ = InvalidateRect(hwnd, None, true);
            }
        }
        ID_WIZARD_NEXT => {
            // Save token from edit control on step 3
            if WIZARD_STEP == 3 {
                let edit = GetDlgItem(hwnd, ID_WIZARD_TOKEN_EDIT).unwrap_or_default();
                if !edit.0.is_null() {
                    let mut buffer = [0u16; 256];
                    let len = GetWindowTextW(edit, &mut buffer);
                    let token = String::from_utf16_lossy(&buffer[..len as usize]);
                    WIZARD_TOKEN = Some(token.trim().to_string());
                }
            }

            if WIZARD_STEP < WIZARD_TOTAL_STEPS {
                WIZARD_STEP += 1;

                // Start polling on step 4
                if WIZARD_STEP == 4 && !WIZARD_POLLING {
                    WIZARD_POLLING = true;
                    SetTimer(hwnd, TIMER_POLL_TELEGRAM, 2000, None);
                }

                // Destroy token edit when leaving step 3
                if WIZARD_STEP == 4 {
                    let edit = GetDlgItem(hwnd, ID_WIZARD_TOKEN_EDIT).unwrap_or_default();
                    if !edit.0.is_null() {
                        DestroyWindow(edit).ok();
                    }
                }

                // On step 5, save config and send test message
                if WIZARD_STEP == 5 {
                    KillTimer(hwnd, TIMER_POLL_TELEGRAM).ok();
                    WIZARD_POLLING = false;
                    save_wizard_config();
                }

                let _ = InvalidateRect(hwnd, None, true);
            } else {
                // Finish
                DestroyWindow(hwnd).ok();
            }
        }
        ID_WIZARD_TOKEN_EDIT => {
            // Token edit changed - update validation
            let edit = GetDlgItem(hwnd, ID_WIZARD_TOKEN_EDIT).unwrap_or_default();
            if !edit.0.is_null() {
                let mut buffer = [0u16; 256];
                let len = GetWindowTextW(edit, &mut buffer);
                let token = String::from_utf16_lossy(&buffer[..len as usize]);
                WIZARD_TOKEN = Some(token.trim().to_string());
                let _ = InvalidateRect(hwnd, None, true);
            }
        }
        _ => {}
    }
}

/// Poll Telegram for incoming messages to detect chat ID
unsafe fn poll_telegram_for_chatid(hwnd: HWND) {
    let wizard_chat_id_ref = std::ptr::addr_of!(WIZARD_CHAT_ID);
    if !WIZARD_POLLING || (*wizard_chat_id_ref).is_some() {
        return;
    }

    if let Some(ref token) = WIZARD_TOKEN {
        // Spawn a thread to do the API call
        let token = token.clone();
        std::thread::spawn(move || {
            if let Some(chat_id) = fetch_telegram_updates(&token) {
                // Post message back to UI thread
                WIZARD_CHAT_ID = Some(chat_id);
                if let Some(wizard_hwnd) = WIZARD_HWND {
                    let _ = InvalidateRect(wizard_hwnd, None, true);
                }
            }
        });
    }

    // Trigger repaint for animation
    let _ = InvalidateRect(hwnd, None, true);
}

/// Fetch latest updates from Telegram to get chat ID
fn fetch_telegram_updates(token: &str) -> Option<i64> {
    let url = format!("https://api.telegram.org/bot{}/getUpdates?limit=1&timeout=1", token);

    // Use ureq for simple HTTP request (we already have it as a dependency through teloxide)
    let response = match ureq::get(&url).timeout(std::time::Duration::from_secs(3)).call() {
        Ok(r) => r,
        Err(_) => return None,
    };

    let body = match response.into_string() {
        Ok(b) => b,
        Err(_) => return None,
    };

    // Parse JSON manually (avoid adding serde dependency just for this)
    // Look for "chat":{"id": pattern
    if let Some(chat_pos) = body.find("\"chat\":{\"id\":") {
        let start = chat_pos + 13;
        if let Some(end_pos) = body[start..].find(|c: char| !c.is_numeric() && c != '-') {
            if let Ok(id) = body[start..start + end_pos].parse::<i64>() {
                return Some(id);
            }
        }
    }

    None
}

/// Save wizard configuration to database
unsafe fn save_wizard_config() {
    let wizard_token_ref = std::ptr::addr_of!(WIZARD_TOKEN);
    let wizard_chat_id_ref = std::ptr::addr_of!(WIZARD_CHAT_ID);
    if let (Some(ref token), Some(chat_id)) = (&*wizard_token_ref, *wizard_chat_id_ref) {
        // Save to database using the existing function signature
        crate::database::set_telegram_config(token, &chat_id.to_string(), true);

        // Send test message
        let token = token.clone();
        std::thread::spawn(move || {
            let _ = send_test_message(&token, chat_id);
        });
    }
}

/// Send a test message to confirm setup
fn send_test_message(token: &str, chat_id: i64) -> Result<(), ()> {
    let message = format!("✅ {} - Screen Time Manager", crate::i18n::t("wizard.success.title"));
    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage?chat_id={}&text={}",
        token,
        chat_id,
        urlencoding::encode(&message)
    );

    let _ = ureq::get(&url).timeout(std::time::Duration::from_secs(5)).call();
    Ok(())
}
