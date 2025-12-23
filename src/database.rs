//! Database module for Screen Time Manager
//! Handles SQLite database initialization and settings management

use std::path::PathBuf;
use std::sync::Mutex;
use rusqlite::{Connection, params};
use windows::core::PCWSTR;

/// Global database connection (thread-safe)
pub static DB_CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);

/// Weekday keys for database
pub const WEEKDAY_KEYS: [&str; 7] = [
    "limit_monday", "limit_tuesday", "limit_wednesday", "limit_thursday",
    "limit_friday", "limit_saturday", "limit_sunday"
];

/// Weekday names for UI
pub const WEEKDAY_NAMES: [&str; 7] = [
    "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"
];

/// Get the path to the database file in a hidden location
pub fn get_database_path() -> PathBuf {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".screen-time-manager");

    if !data_dir.exists() {
        let _ = std::fs::create_dir_all(&data_dir);

        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let path: Vec<u16> = data_dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            unsafe {
                let _ = windows::Win32::Storage::FileSystem::SetFileAttributesW(
                    PCWSTR(path.as_ptr()),
                    windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_HIDDEN,
                );
            }
        }
    }

    data_dir.join("data.db")
}

/// Initialize the SQLite database
pub fn init_database() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_database_path();
    let conn = Connection::open(&db_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;

    // Default settings to initialize
    let defaults = [
        ("passcode", "0000"),
        // Daily limits in minutes (default 120 = 2 hours)
        ("limit_monday", "120"),
        ("limit_tuesday", "120"),
        ("limit_wednesday", "120"),
        ("limit_thursday", "120"),
        ("limit_friday", "180"),
        ("limit_saturday", "240"),
        ("limit_sunday", "240"),
        // First warning (minutes before limit)
        ("warning1_minutes", "10"),
        ("warning1_message", "10 minutes remaining!"),
        // Second warning (minutes before limit)
        ("warning2_minutes", "5"),
        ("warning2_message", "5 minutes remaining!"),
        // Blocking message
        ("blocking_message", "Your screen time limit has been reached."),
    ];

    for (key, value) in defaults {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings WHERE key = ?1)",
            params![key],
            |row| row.get(0),
        )?;

        if !exists {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }
    }

    *DB_CONNECTION.lock().unwrap() = Some(conn);
    Ok(())
}

/// Get the passcode from the database
pub fn get_passcode() -> Option<String> {
    let guard = DB_CONNECTION.lock().ok()?;
    guard.as_ref()?.query_row(
        "SELECT value FROM settings WHERE key = 'passcode'",
        [],
        |row| row.get(0),
    ).ok()
}

/// Set the passcode in the database
#[allow(dead_code)]
pub fn set_passcode(code: &str) -> bool {
    if let Ok(guard) = DB_CONNECTION.lock() {
        if let Some(conn) = guard.as_ref() {
            return conn.execute(
                "UPDATE settings SET value = ?1 WHERE key = 'passcode'",
                params![code],
            ).is_ok();
        }
    }
    false
}

/// Get a setting value from the database
pub fn get_setting(key: &str) -> Option<String> {
    let guard = DB_CONNECTION.lock().ok()?;
    guard.as_ref()?.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    ).ok()
}

/// Set a setting value in the database
pub fn set_setting(key: &str, value: &str) -> bool {
    if let Ok(guard) = DB_CONNECTION.lock() {
        if let Some(conn) = guard.as_ref() {
            return conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            ).is_ok();
        }
    }
    false
}

/// Get daily limit for a specific weekday (0 = Monday, 6 = Sunday)
#[allow(dead_code)]
pub fn get_daily_limit(weekday: u32) -> u32 {
    let key = match weekday {
        0 => "limit_monday",
        1 => "limit_tuesday",
        2 => "limit_wednesday",
        3 => "limit_thursday",
        4 => "limit_friday",
        5 => "limit_saturday",
        6 => "limit_sunday",
        _ => return 120,
    };
    get_setting(key)
        .and_then(|s| s.parse().ok())
        .unwrap_or(120)
}

/// Get warning configuration
#[allow(dead_code)]
pub fn get_warning_config(warning_num: u32) -> (u32, String) {
    let minutes_key = format!("warning{}_minutes", warning_num);
    let message_key = format!("warning{}_message", warning_num);

    let minutes = get_setting(&minutes_key)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let message = get_setting(&message_key)
        .unwrap_or_else(|| format!("{} minutes remaining!", minutes));

    (minutes, message)
}

/// Get blocking message
#[allow(dead_code)]
pub fn get_blocking_message() -> String {
    get_setting("blocking_message")
        .unwrap_or_else(|| "Your screen time limit has been reached.".to_string())
}

/// Get the current local date as a string (YYYY-MM-DD)
fn get_today_date() -> String {
    use windows::Win32::System::SystemInformation::GetLocalTime;

    let st = unsafe { GetLocalTime() };

    format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay)
}

/// Save remaining time to database (associated with current date)
pub fn save_remaining_time(seconds: i32) {
    let date = get_today_date();
    let key = format!("remaining_time_{}", date);
    set_setting(&key, &seconds.to_string());
}

/// Load remaining time from database for today
#[allow(dead_code)]
pub fn load_remaining_time() -> Option<i32> {
    let date = get_today_date();
    let key = format!("remaining_time_{}", date);
    get_setting(&key).and_then(|s| s.parse().ok())
}

/// Get the current weekday (0 = Monday, 6 = Sunday)
#[allow(dead_code)]
pub fn get_current_weekday() -> u32 {
    use windows::Win32::System::SystemInformation::GetLocalTime;

    let st = unsafe { GetLocalTime() };

    // Windows: wDayOfWeek is 0 = Sunday, 1 = Monday, ..., 6 = Saturday
    // We want: 0 = Monday, 1 = Tuesday, ..., 6 = Sunday
    if st.wDayOfWeek == 0 {
        6 // Sunday
    } else {
        (st.wDayOfWeek - 1) as u32
    }
}
