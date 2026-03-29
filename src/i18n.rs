//! Internationalization (i18n) module for Screen Time Manager
//! Supports English (default) and German

use crate::database;
use windows::core::PCWSTR;

/// Convert a translated string to a Windows wide string (null-terminated Vec<u16>)
/// The returned Vec must be kept alive while the PCWSTR is in use
pub fn wide(key: &str) -> Vec<u16> {
    t(key).encode_utf16().chain(std::iter::once(0)).collect()
}

/// Convert a raw string to a Windows wide string
#[allow(dead_code)]
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Get PCWSTR from a wide string Vec (helper for cleaner code)
#[allow(dead_code)]
pub fn pcwstr(wide: &[u16]) -> PCWSTR {
    PCWSTR(wide.as_ptr())
}

/// Supported languages
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Language {
    English,
    German,
}

impl Language {
    /// Get the language code (for storage)
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::German => "de",
        }
    }

    /// Create Language from code string
    pub fn from_code(code: &str) -> Self {
        match code {
            "de" => Language::German,
            _ => Language::English,
        }
    }

    /// Get the display name in the native language
    pub fn name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::German => "Deutsch",
        }
    }

    /// Get all supported languages
    pub fn all() -> &'static [Language] {
        &[Language::English, Language::German]
    }
}

/// Get the current language from settings
pub fn current() -> Language {
    Language::from_code(&database::get_setting("language").unwrap_or_default())
}

/// Set the current language
pub fn set_language(lang: Language) {
    database::set_setting("language", lang.code());
}

/// Main translation function - returns static string for the given key
pub fn t(key: &str) -> &'static str {
    match current() {
        Language::English => en(key),
        Language::German => de(key),
    }
}

/// Get weekday name by index (0 = Monday, 6 = Sunday)
pub fn weekday(index: usize) -> &'static str {
    const EN_DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    const DE_DAYS: [&str; 7] = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"];

    let days = match current() {
        Language::English => &EN_DAYS,
        Language::German => &DE_DAYS,
    };
    days.get(index).unwrap_or(&"Unknown")
}

/// Get short weekday name by index (0 = Monday, 6 = Sunday)
#[allow(dead_code)]
pub fn weekday_short(index: usize) -> &'static str {
    const EN_DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const DE_DAYS: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];

    let days = match current() {
        Language::English => &EN_DAYS,
        Language::German => &DE_DAYS,
    };
    days.get(index).unwrap_or(&"?")
}

// ============================================================================
// English strings
// ============================================================================

fn en(key: &str) -> &'static str {
    match key {
        // ----- Window Titles -----
        "window.settings" => "Screen Time Settings",
        "window.passcode" => "Enter Passcode",
        "window.stats" => "Today's Stats",
        "window.blocking" => "Screen Time - Time's Up!",
        "window.about" => "About",

        // ----- Settings Dialog - Section Titles -----
        "settings.daily_limits" => "Daily Time Limits (minutes)",
        "settings.warning1" => "First Warning",
        "settings.warning2" => "Second Warning",
        "settings.blocking_message" => "Blocking Screen Message",
        "settings.passcode" => "Change Passcode (leave blank to keep)",
        "settings.telegram" => "Telegram Bot",
        "settings.lock_screen" => "Lock Screen",
        "settings.idle" => "Idle Detection",
        "settings.language" => "Language",

        // ----- Settings Dialog - Labels -----
        "settings.minutes_before" => "Minutes before:",
        "settings.message" => "Message:",
        "settings.current" => "Current:",
        "settings.new" => "New:",
        "settings.confirm" => "Confirm:",
        "settings.enable_telegram" => "Enable Telegram Bot",
        "settings.bot_token" => "Bot Token:",
        "settings.chat_id" => "Chat ID:",
        "settings.setup_wizard" => "Setup Wizard...",
        "settings.shutdown_timeout" => "Shutdown timeout:",
        "settings.auto_pause_idle" => "Auto-pause when idle",
        "settings.idle_timeout" => "Idle timeout (min):",

        // ----- Settings Dialog - Buttons -----
        "button.save" => "Save",
        "button.cancel" => "Cancel",
        "button.ok" => "OK",
        "button.close" => "Close",
        "button.reset_timer" => "Reset Timer",

        // ----- Settings Dialog - Messages -----
        "settings.error.current_incorrect" => "Current passcode is incorrect!",
        "settings.error.passcode_length" => "New passcode must be exactly 4 digits!",
        "settings.error.passcode_mismatch" => "New passcode and confirmation do not match!",
        "settings.success.saved" => "Settings saved successfully!",
        "settings.error" => "Error",
        "settings.success" => "Settings",

        // ----- Passcode Dialog -----
        "passcode.subtitle" => "Enter 4-digit code to continue",
        "passcode.incorrect" => "Incorrect passcode",

        // ----- Stats Dialog -----
        "stats.title" => "Today's Statistics",
        "stats.day" => "Day:",
        "stats.daily_limit" => "Daily Limit:",
        "stats.time_used" => "Time Used:",
        "stats.time_remaining" => "Time Remaining:",
        "stats.pause_mode" => "Pause Mode",
        "stats.pause_used" => "Pause Used:",
        "stats.pause_remaining" => "Pause Remaining:",
        "stats.pauses_today" => "Pauses Today:",
        "stats.log" => "Log:",
        "stats.pause_disabled" => "Pause feature is disabled",
        "stats.timer_reset" => "Timer has been reset to the daily limit.",
        "stats.timer_reset_title" => "Timer Reset",

        // ----- Tray Menu -----
        "tray.tooltip" => "Screen Time Manager",
        "tray.stats" => "Today's Stats...",
        "tray.settings" => "Settings...",
        "tray.extend_15" => "Extend +15 min",
        "tray.extend_45" => "Extend +45 min",
        "tray.resume" => "Resume Timer",
        "tray.pause_idle" => "Pause (Idle paused)",
        "tray.pause_disabled" => "Pause (Disabled)",
        "tray.pause_budget_used" => "Pause (Budget used)",
        "tray.pause_time_low" => "Pause (Time too low)",
        "tray.idle_paused" => "Idle: Paused",
        "tray.show_warning" => "Show Warning (5s)",
        "tray.show_blocking" => "Show Blocking Overlay",
        "tray.about" => "About",
        "tray.quit" => "Quit",

        // ----- Blocking Screen -----
        "blocking.times_up" => "Time's Up!",
        "blocking.limit_reached" => "Screen time limit reached",
        "blocking.extend_label" => "Extend time (requires passcode):",
        "blocking.passcode_label" => "Enter passcode to unlock:",
        "blocking.incorrect" => "Incorrect passcode!",
        "blocking.shutdown_in" => "Shutdown in:",
        "blocking.shutdown_now" => "SHUTDOWN IN:",
        "blocking.time_exceeded" => "Time limit exceeded",
        "blocking.extend_15" => "+15 min",
        "blocking.extend_30" => "+30 min",
        "blocking.extend_60" => "+60 min",
        "blocking.unlock" => "Unlock",
        "blocking.shutdown" => "Shut Down",
        "blocking.confirm_shutdown" => "Are you sure you want to shut down the computer?",
        "blocking.confirm_title" => "Confirm Shutdown",
        "blocking.screen_locked" => "Screen Locked",

        // ----- About Dialog -----
        "about.text" => "Screen Time Manager v1.0.39\n\nA parental control application for managing screen time.\n\n(c) Simon Pamies",

        // ----- Pause Reasons -----
        "pause.disabled" => "Pause feature is disabled",
        "pause.budget_exhausted" => "Daily pause budget exhausted",
        "pause.cooldown" => "Cooldown active",
        "pause.min_active" => "Need more active time",
        "pause.time_too_low" => "Time is too low to pause",

        // ----- Telegram Bot - Command Descriptions -----
        "tg.cmd.start" => "Start the bot",
        "tg.cmd.status" => "Show remaining time and status",
        "tg.cmd.time" => "Quick time check",
        "tg.cmd.extend" => "Extend time by minutes (e.g., /extend 30)",
        "tg.cmd.reduce" => "Reduce time by minutes (e.g., /reduce 30)",
        "tg.cmd.pause" => "Pause the timer",
        "tg.cmd.resume" => "Resume the timer",
        "tg.cmd.history" => "Show today's pause activity",
        "tg.cmd.msg" => "Show a message on screen (e.g., /msg Do your homework!)",
        "tg.cmd.lock" => "Lock the screen",
        "tg.cmd.stop" => "Lock the screen (alias)",
        "tg.cmd.reset" => "Reset timer to daily limit",
        "tg.cmd.e30" => "Extend by 30 minutes",
        "tg.cmd.e60" => "Extend by 60 minutes",
        "tg.cmd.e120" => "Extend by 120 minutes",
        "tg.cmd.chatid" => "Get your chat ID for setup",
        "tg.cmd.help" => "Show this help message",

        // ----- Telegram Bot - Responses -----
        "tg.status.header" => "Screen Time Status",
        "tg.status.remaining" => "Remaining:",
        "tg.status.paused" => "Paused:",
        "tg.status.pause_budget" => "Pause budget:",
        "tg.status.yes" => "Yes",
        "tg.status.no" => "No",
        "tg.status.idle" => "Yes (idle)",

        "tg.extend.specify_positive" => "Please specify a positive number of minutes",
        "tg.extend.max_120" => "Maximum extension is 120 minutes",
        "tg.extend.success" => "Extended by {} minutes\nNew remaining:",

        "tg.reduce.specify_positive" => "Please specify a positive number of minutes",
        "tg.reduce.max_120" => "Maximum reduction is 120 minutes",
        "tg.reduce.not_enough" => "Cannot reduce - not enough time remaining",
        "tg.reduce.success" => "Reduced by {} minutes\nNew remaining:",

        "tg.pause.already_paused" => "Timer is already paused. Use /resume to continue.",
        "tg.pause.idle_paused" => "Timer is already paused (idle). It will resume automatically when input is detected.",
        "tg.pause.success" => "Timer paused",
        "tg.pause.failed" => "Timer was not paused (unexpected state)",
        "tg.pause.cannot" => "Cannot pause:",

        "tg.resume.idle_auto" => "Timer is idle-paused. It will resume automatically when input is detected.",
        "tg.resume.not_paused" => "Timer is not paused",
        "tg.resume.success" => "Timer resumed",
        "tg.resume.failed" => "Timer is still paused (unexpected state)",
        "tg.resume.cannot" => "Cannot resume:",

        "tg.history.header" => "Today's Activity",
        "tg.history.uptime" => "Uptime:",
        "tg.history.pause_used" => "Pause used:",
        "tg.history.no_events" => "No pause events today",

        "tg.msg.provide" => "Please provide a message, e.g. /msg Do your homework!",
        "tg.msg.shown" => "Message shown:",

        "tg.reset.success" => "Timer reset to daily limit",
        "tg.reset.remaining" => "Remaining:",

        "tg.lock.success" => "Screen locked",

        "tg.error.unknown_cmd" => "Unknown command. Use /help to see available commands.",
        "tg.error.unauthorized" => "Unauthorized. This bot is configured for a specific user.",
        "tg.error.no_admin" => "No admin configured. Please set your chat ID in settings.",
        "tg.chatid.your_id" => "Your chat ID is:",

        "tg.notify.started" => "Screen Time Manager started",
        "tg.notify.shutdown" => "Screen Time Manager is shutting down",

        // ----- Telegram Setup Wizard -----
        "wizard.title" => "Telegram Setup Wizard",
        "wizard.step" => "Step",
        "wizard.of" => "of",
        "wizard.next" => "Next",
        "wizard.back" => "Back",
        "wizard.finish" => "Finish",
        "wizard.cancel" => "Cancel",
        "wizard.skip" => "Skip",

        // Step 1: Welcome
        "wizard.welcome.title" => "Remote Control via Telegram",
        "wizard.welcome.desc1" => "Control Screen Time Manager from your phone!",
        "wizard.welcome.desc2" => "With Telegram you can:",
        "wizard.welcome.feature1" => "Check remaining time",
        "wizard.welcome.feature2" => "Extend or reduce time remotely",
        "wizard.welcome.feature3" => "Lock the screen instantly",
        "wizard.welcome.feature4" => "Receive notifications",
        "wizard.welcome.ready" => "Let's set it up in 3 easy steps!",

        // Step 2: Create Bot
        "wizard.bot.title" => "Create Your Bot",
        "wizard.bot.step1" => "1. Open Telegram on your phone",
        "wizard.bot.step2" => "2. Search for  @BotFather",
        "wizard.bot.step3" => "3. Send the message:  /newbot",
        "wizard.bot.step4" => "4. Choose a name (e.g. \"My Screen Time\")",
        "wizard.bot.step5" => "5. Choose a username ending in 'bot'",
        "wizard.bot.step6" => "6. BotFather will give you a token - copy it!",
        "wizard.bot.hint" => "The token looks like: 123456789:ABCdef...",

        // Step 3: Enter Token
        "wizard.token.title" => "Enter Your Bot Token",
        "wizard.token.label" => "Paste the token from BotFather:",
        "wizard.token.placeholder" => "123456789:ABCdefGHI...",
        "wizard.token.invalid" => "This doesn't look like a valid token",
        "wizard.token.valid" => "Token looks good!",

        // Step 4: Connect
        "wizard.connect.title" => "Connect to Your Bot",
        "wizard.connect.step1" => "1. Open Telegram",
        "wizard.connect.step2" => "2. Search for your new bot",
        "wizard.connect.step3" => "3. Press START or send any message",
        "wizard.connect.waiting" => "Waiting for your message...",
        "wizard.connect.detected" => "Connection detected!",
        "wizard.connect.chatid" => "Your Chat ID:",

        // Step 5: Success
        "wizard.success.title" => "Setup Complete!",
        "wizard.success.desc" => "Your Telegram bot is ready to use.",
        "wizard.success.test" => "A test message was sent to your phone.",
        "wizard.success.commands" => "Try these commands in Telegram:",
        "wizard.success.cmd1" => "/status - Check remaining time",
        "wizard.success.cmd2" => "/extend 30 - Add 30 minutes",
        "wizard.success.cmd3" => "/lock - Lock the screen",
        "wizard.success.cmd4" => "/help - See all commands",

        // Fallback - return empty string for unknown keys (should not happen in practice)
        _ => "",
    }
}

// ============================================================================
// German strings
// ============================================================================

fn de(key: &str) -> &'static str {
    match key {
        // ----- Window Titles -----
        "window.settings" => "Bildschirmzeit Einstellungen",
        "window.passcode" => "Code eingeben",
        "window.stats" => "Heutige Statistik",
        "window.blocking" => "Bildschirmzeit - Zeit abgelaufen!",
        "window.about" => "Info",

        // ----- Settings Dialog - Section Titles -----
        "settings.daily_limits" => "Tägliche Zeitlimits (Minuten)",
        "settings.warning1" => "Erste Warnung",
        "settings.warning2" => "Zweite Warnung",
        "settings.blocking_message" => "Sperrbildschirm-Nachricht",
        "settings.passcode" => "Code ändern (leer lassen zum Behalten)",
        "settings.telegram" => "Telegram Bot",
        "settings.lock_screen" => "Bildschirmsperre",
        "settings.idle" => "Leerlauferkennung",
        "settings.language" => "Sprache",

        // ----- Settings Dialog - Labels -----
        "settings.minutes_before" => "Minuten vorher:",
        "settings.message" => "Nachricht:",
        "settings.current" => "Aktuell:",
        "settings.new" => "Neu:",
        "settings.confirm" => "Bestätigen:",
        "settings.enable_telegram" => "Telegram Bot aktivieren",
        "settings.bot_token" => "Bot Token:",
        "settings.chat_id" => "Chat ID:",
        "settings.setup_wizard" => "Einrichtungsassistent...",
        "settings.shutdown_timeout" => "Abschaltzeit:",
        "settings.auto_pause_idle" => "Auto-Pause bei Leerlauf",
        "settings.idle_timeout" => "Leerlaufzeit (Min):",

        // ----- Settings Dialog - Buttons -----
        "button.save" => "Speichern",
        "button.cancel" => "Abbrechen",
        "button.ok" => "OK",
        "button.close" => "Schließen",
        "button.reset_timer" => "Timer zurücksetzen",

        // ----- Settings Dialog - Messages -----
        "settings.error.current_incorrect" => "Aktueller Code ist falsch!",
        "settings.error.passcode_length" => "Neuer Code muss genau 4 Ziffern haben!",
        "settings.error.passcode_mismatch" => "Neuer Code und Bestätigung stimmen nicht überein!",
        "settings.success.saved" => "Einstellungen erfolgreich gespeichert!",
        "settings.error" => "Fehler",
        "settings.success" => "Einstellungen",

        // ----- Passcode Dialog -----
        "passcode.subtitle" => "4-stelligen Code eingeben",
        "passcode.incorrect" => "Falscher Code",

        // ----- Stats Dialog -----
        "stats.title" => "Heutige Statistik",
        "stats.day" => "Tag:",
        "stats.daily_limit" => "Tageslimit:",
        "stats.time_used" => "Zeit genutzt:",
        "stats.time_remaining" => "Zeit verbleibend:",
        "stats.pause_mode" => "Pause-Modus",
        "stats.pause_used" => "Pause genutzt:",
        "stats.pause_remaining" => "Pause verbleibend:",
        "stats.pauses_today" => "Pausen heute:",
        "stats.log" => "Protokoll:",
        "stats.pause_disabled" => "Pause-Funktion ist deaktiviert",
        "stats.timer_reset" => "Timer wurde auf das Tageslimit zurückgesetzt.",
        "stats.timer_reset_title" => "Timer zurückgesetzt",

        // ----- Tray Menu -----
        "tray.tooltip" => "Bildschirmzeit Manager",
        "tray.stats" => "Heutige Statistik...",
        "tray.settings" => "Einstellungen...",
        "tray.extend_15" => "+15 Min verlängern",
        "tray.extend_45" => "+45 Min verlängern",
        "tray.resume" => "Timer fortsetzen",
        "tray.pause_idle" => "Pause (Leerlauf)",
        "tray.pause_disabled" => "Pause (Deaktiviert)",
        "tray.pause_budget_used" => "Pause (Budget aufgebraucht)",
        "tray.pause_time_low" => "Pause (Zeit zu niedrig)",
        "tray.idle_paused" => "Leerlauf: Pausiert",
        "tray.show_warning" => "Warnung anzeigen (5s)",
        "tray.show_blocking" => "Sperrbildschirm anzeigen",
        "tray.about" => "Info",
        "tray.quit" => "Beenden",

        // ----- Blocking Screen -----
        "blocking.times_up" => "Zeit abgelaufen!",
        "blocking.limit_reached" => "Bildschirmzeit-Limit erreicht",
        "blocking.extend_label" => "Zeit verlängern (Code erforderlich):",
        "blocking.passcode_label" => "Code zum Entsperren eingeben:",
        "blocking.incorrect" => "Falscher Code!",
        "blocking.shutdown_in" => "Herunterfahren in:",
        "blocking.shutdown_now" => "HERUNTERFAHREN IN:",
        "blocking.time_exceeded" => "Zeitlimit überschritten",
        "blocking.extend_15" => "+15 Min",
        "blocking.extend_30" => "+30 Min",
        "blocking.extend_60" => "+60 Min",
        "blocking.unlock" => "Entsperren",
        "blocking.shutdown" => "Herunterfahren",
        "blocking.confirm_shutdown" => "Möchten Sie den Computer wirklich herunterfahren?",
        "blocking.confirm_title" => "Herunterfahren bestätigen",
        "blocking.screen_locked" => "Bildschirm gesperrt",

        // ----- About Dialog -----
        "about.text" => "Bildschirmzeit Manager v1.0.39\n\nEine Kindersicherungs-App zur Verwaltung der Bildschirmzeit.\n\n(c) Simon Pamies",

        // ----- Pause Reasons -----
        "pause.disabled" => "Pause-Funktion ist deaktiviert",
        "pause.budget_exhausted" => "Tägliches Pause-Budget aufgebraucht",
        "pause.cooldown" => "Abklingzeit aktiv",
        "pause.min_active" => "Mehr aktive Zeit erforderlich",
        "pause.time_too_low" => "Zeit zu niedrig für Pause",

        // ----- Telegram Bot - Command Descriptions -----
        "tg.cmd.start" => "Bot starten",
        "tg.cmd.status" => "Verbleibende Zeit und Status anzeigen",
        "tg.cmd.time" => "Schnelle Zeitabfrage",
        "tg.cmd.extend" => "Zeit verlängern (z.B. /extend 30)",
        "tg.cmd.reduce" => "Zeit verringern (z.B. /reduce 30)",
        "tg.cmd.pause" => "Timer pausieren",
        "tg.cmd.resume" => "Timer fortsetzen",
        "tg.cmd.history" => "Heutige Pause-Aktivität anzeigen",
        "tg.cmd.msg" => "Nachricht anzeigen (z.B. /msg Mach deine Hausaufgaben!)",
        "tg.cmd.lock" => "Bildschirm sperren",
        "tg.cmd.stop" => "Bildschirm sperren (Alias)",
        "tg.cmd.reset" => "Timer auf Tageslimit zurücksetzen",
        "tg.cmd.e30" => "Um 30 Minuten verlängern",
        "tg.cmd.e60" => "Um 60 Minuten verlängern",
        "tg.cmd.e120" => "Um 120 Minuten verlängern",
        "tg.cmd.chatid" => "Chat-ID für Einrichtung abrufen",
        "tg.cmd.help" => "Diese Hilfe anzeigen",

        // ----- Telegram Bot - Responses -----
        "tg.status.header" => "Bildschirmzeit Status",
        "tg.status.remaining" => "Verbleibend:",
        "tg.status.paused" => "Pausiert:",
        "tg.status.pause_budget" => "Pause-Budget:",
        "tg.status.yes" => "Ja",
        "tg.status.no" => "Nein",
        "tg.status.idle" => "Ja (Leerlauf)",

        "tg.extend.specify_positive" => "Bitte geben Sie eine positive Minutenzahl an",
        "tg.extend.max_120" => "Maximale Verlängerung ist 120 Minuten",
        "tg.extend.success" => "Um {} Minuten verlängert\nNeu verbleibend:",

        "tg.reduce.specify_positive" => "Bitte geben Sie eine positive Minutenzahl an",
        "tg.reduce.max_120" => "Maximale Verringerung ist 120 Minuten",
        "tg.reduce.not_enough" => "Kann nicht verringern - nicht genug Zeit verbleibend",
        "tg.reduce.success" => "Um {} Minuten verringert\nNeu verbleibend:",

        "tg.pause.already_paused" => "Timer ist bereits pausiert. Verwenden Sie /resume zum Fortsetzen.",
        "tg.pause.idle_paused" => "Timer ist bereits pausiert (Leerlauf). Er wird automatisch fortgesetzt, wenn Eingabe erkannt wird.",
        "tg.pause.success" => "Timer pausiert",
        "tg.pause.failed" => "Timer wurde nicht pausiert (unerwarteter Zustand)",
        "tg.pause.cannot" => "Kann nicht pausieren:",

        "tg.resume.idle_auto" => "Timer ist im Leerlauf pausiert. Er wird automatisch fortgesetzt, wenn Eingabe erkannt wird.",
        "tg.resume.not_paused" => "Timer ist nicht pausiert",
        "tg.resume.success" => "Timer fortgesetzt",
        "tg.resume.failed" => "Timer ist noch pausiert (unerwarteter Zustand)",
        "tg.resume.cannot" => "Kann nicht fortsetzen:",

        "tg.history.header" => "Heutige Aktivität",
        "tg.history.uptime" => "Laufzeit:",
        "tg.history.pause_used" => "Pause genutzt:",
        "tg.history.no_events" => "Keine Pause-Ereignisse heute",

        "tg.msg.provide" => "Bitte geben Sie eine Nachricht an, z.B. /msg Mach deine Hausaufgaben!",
        "tg.msg.shown" => "Nachricht angezeigt:",

        "tg.reset.success" => "Timer auf Tageslimit zurückgesetzt",
        "tg.reset.remaining" => "Verbleibend:",

        "tg.lock.success" => "Bildschirm gesperrt",

        "tg.error.unknown_cmd" => "Unbekannter Befehl. Verwenden Sie /help für verfügbare Befehle.",
        "tg.error.unauthorized" => "Nicht autorisiert. Dieser Bot ist für einen bestimmten Benutzer konfiguriert.",
        "tg.error.no_admin" => "Kein Admin konfiguriert. Bitte setzen Sie Ihre Chat-ID in den Einstellungen.",
        "tg.chatid.your_id" => "Ihre Chat-ID ist:",

        "tg.notify.started" => "Bildschirmzeit Manager gestartet",
        "tg.notify.shutdown" => "Bildschirmzeit Manager wird heruntergefahren",

        // ----- Telegram Setup Wizard -----
        "wizard.title" => "Telegram Einrichtungsassistent",
        "wizard.step" => "Schritt",
        "wizard.of" => "von",
        "wizard.next" => "Weiter",
        "wizard.back" => "Zurück",
        "wizard.finish" => "Fertig",
        "wizard.cancel" => "Abbrechen",
        "wizard.skip" => "Überspringen",

        // Step 1: Welcome
        "wizard.welcome.title" => "Fernsteuerung via Telegram",
        "wizard.welcome.desc1" => "Steuern Sie den Bildschirmzeit Manager vom Handy!",
        "wizard.welcome.desc2" => "Mit Telegram können Sie:",
        "wizard.welcome.feature1" => "Verbleibende Zeit prüfen",
        "wizard.welcome.feature2" => "Zeit ferngesteuert verlängern oder verkürzen",
        "wizard.welcome.feature3" => "Bildschirm sofort sperren",
        "wizard.welcome.feature4" => "Benachrichtigungen erhalten",
        "wizard.welcome.ready" => "Richten wir es in 3 einfachen Schritten ein!",

        // Step 2: Create Bot
        "wizard.bot.title" => "Erstellen Sie Ihren Bot",
        "wizard.bot.step1" => "1. Öffnen Sie Telegram auf Ihrem Handy",
        "wizard.bot.step2" => "2. Suchen Sie nach  @BotFather",
        "wizard.bot.step3" => "3. Senden Sie die Nachricht:  /newbot",
        "wizard.bot.step4" => "4. Wählen Sie einen Namen (z.B. \"Meine Bildschirmzeit\")",
        "wizard.bot.step5" => "5. Wählen Sie einen Benutzernamen mit 'bot' am Ende",
        "wizard.bot.step6" => "6. BotFather gibt Ihnen einen Token - kopieren Sie ihn!",
        "wizard.bot.hint" => "Der Token sieht so aus: 123456789:ABCdef...",

        // Step 3: Enter Token
        "wizard.token.title" => "Bot-Token eingeben",
        "wizard.token.label" => "Fügen Sie den Token von BotFather ein:",
        "wizard.token.placeholder" => "123456789:ABCdefGHI...",
        "wizard.token.invalid" => "Das sieht nicht wie ein gültiger Token aus",
        "wizard.token.valid" => "Token sieht gut aus!",

        // Step 4: Connect
        "wizard.connect.title" => "Mit Ihrem Bot verbinden",
        "wizard.connect.step1" => "1. Öffnen Sie Telegram",
        "wizard.connect.step2" => "2. Suchen Sie nach Ihrem neuen Bot",
        "wizard.connect.step3" => "3. Drücken Sie START oder senden Sie eine Nachricht",
        "wizard.connect.waiting" => "Warte auf Ihre Nachricht...",
        "wizard.connect.detected" => "Verbindung erkannt!",
        "wizard.connect.chatid" => "Ihre Chat-ID:",

        // Step 5: Success
        "wizard.success.title" => "Einrichtung abgeschlossen!",
        "wizard.success.desc" => "Ihr Telegram-Bot ist einsatzbereit.",
        "wizard.success.test" => "Eine Testnachricht wurde an Ihr Handy gesendet.",
        "wizard.success.commands" => "Probieren Sie diese Befehle in Telegram:",
        "wizard.success.cmd1" => "/status - Verbleibende Zeit prüfen",
        "wizard.success.cmd2" => "/extend 30 - 30 Minuten hinzufügen",
        "wizard.success.cmd3" => "/lock - Bildschirm sperren",
        "wizard.success.cmd4" => "/help - Alle Befehle anzeigen",

        // Fallback to English
        _ => en(key),
    }
}
