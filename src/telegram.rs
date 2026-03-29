//! Telegram bot module for Screen Time Manager
//! Provides remote monitoring and control via Telegram commands

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use teloxide::prelude::*;
use teloxide::error_handlers::LoggingErrorHandler;
use teloxide::utils::command::BotCommands;

use crate::blocking;
use crate::database;
use crate::i18n;
use crate::mini_overlay;
use crate::overlay;

/// Shutdown signal for graceful termination
pub static BOT_SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Bot instance for sending notifications
static BOT_INSTANCE: OnceLock<Bot> = OnceLock::new();

/// Admin chat ID for notifications
static ADMIN_CHAT_ID: OnceLock<i64> = OnceLock::new();

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "lowercase", description = "Screen Time Manager commands:")]
enum Command {
    #[command(description = "Start the bot")]
    Start,
    #[command(description = "Show remaining time and status")]
    Status,
    #[command(description = "Quick time check")]
    Time,
    #[command(description = "Extend time by minutes (e.g., /extend 30)")]
    Extend(i32),
    #[command(description = "Reduce time by minutes (e.g., /reduce 30)")]
    Reduce(i32),
    #[command(description = "Pause the timer")]
    Pause,
    #[command(description = "Resume the timer")]
    Resume,
    #[command(description = "Show today's pause activity")]
    History,
    #[command(description = "Show a message on screen (e.g., /msg Do your homework!)")]
    Msg(String),
    #[command(description = "Lock the screen")]
    Lock,
    #[command(description = "Lock the screen (alias)")]
    Stop,
    #[command(description = "Reset timer to daily limit")]
    Reset,
    #[command(description = "Extend by 30 minutes")]
    E30,
    #[command(description = "Extend by 60 minutes")]
    E60,
    #[command(description = "Extend by 120 minutes")]
    E120,
    #[command(description = "Get your chat ID for setup")]
    Chatid,
    #[command(description = "Show this help message")]
    Help,
}

/// Start the Telegram bot in a background thread
pub fn start_bot_thread() {
    let config = database::get_telegram_config();

    if !config.enabled {
        eprintln!("[Telegram] Bot is disabled in settings");
        return;
    }

    let Some(token) = config.bot_token else {
        eprintln!("[Telegram] Bot enabled but no token configured");
        return;
    };

    if token.is_empty() {
        eprintln!("[Telegram] Bot token is empty");
        return;
    }

    let admin_chat_id = config.admin_chat_id;

    // Store admin chat ID for notifications
    if let Some(id) = admin_chat_id {
        let _ = ADMIN_CHAT_ID.set(id);
    }

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            run_bot(token, admin_chat_id).await;
        });
    });
}

/// Signal the bot to shut down gracefully
pub fn signal_shutdown() {
    BOT_SHUTDOWN.store(true, Ordering::SeqCst);

    // Send shutdown notification if possible
    if let (Some(bot), Some(&chat_id)) = (BOT_INSTANCE.get(), ADMIN_CHAT_ID.get()) {
        let bot = bot.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok();
            if let Some(rt) = rt {
                rt.block_on(async {
                    let _ = bot.send_message(ChatId(chat_id), i18n::t("tg.notify.shutdown")).await;
                });
            }
        });
        // Give a moment for the message to send
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Main bot loop
async fn run_bot(token: String, admin_chat_id: Option<i64>) {
    let bot = Bot::new(&token);

    // Store bot instance for notifications
    let _ = BOT_INSTANCE.set(bot.clone());

    // Send startup notification
    if let Some(chat_id) = admin_chat_id {
        let _ = bot.send_message(ChatId(chat_id), i18n::t("tg.notify.started")).await;
    }

    // Command handler
    let command_handler = Update::filter_message()
        .filter_command::<Command>()
        .endpoint(move |bot: Bot, msg: Message, cmd: Command| {
            handle_command(bot, msg, cmd, admin_chat_id)
        });

    // Fallback handler: show plain text as on-screen message (authorized users only)
    let fallback_handler = Update::filter_message()
        .endpoint(move |bot: Bot, msg: Message| async move {
            if let Some(text) = msg.text() {
                if text.starts_with('/') {
                    bot.send_message(
                        msg.chat.id,
                        i18n::t("tg.error.unknown_cmd")
                    ).await?;
                } else if !text.is_empty() {
                    // Check authorization
                    let authorized = admin_chat_id
                        .map(|id| msg.chat.id.0 == id)
                        .unwrap_or(false);
                    if authorized {
                        unsafe {
                            overlay::show_overlay(text, 10);
                        }
                        bot.send_message(
                            msg.chat.id,
                            format!("📢 {}: \"{}\"", i18n::t("tg.msg.shown"), text)
                        ).await?;
                    }
                }
            }
            Ok(())
        });

    // Combine handlers - commands first, then fallback
    let handler = dptree::entry()
        .branch(command_handler)
        .branch(fallback_handler);

    // Create dispatcher with default error handler that logs errors
    let mut dispatcher = Dispatcher::builder(bot, handler)
        .default_handler(|upd| async move {
            eprintln!("[Telegram] Unhandled update: {:?}", upd);
        })
        .error_handler(LoggingErrorHandler::with_custom_text("[Telegram] Error in handler"))
        .build();

    // Get shutdown token for graceful shutdown
    let shutdown_token = dispatcher.shutdown_token();

    // Spawn a task to monitor shutdown signal
    tokio::spawn(async move {
        while !BOT_SHUTDOWN.load(Ordering::SeqCst) {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        shutdown_token.shutdown().ok();
    });

    // Run dispatcher
    dispatcher.dispatch().await;
}

/// Handle incoming commands
async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    admin_chat_id: Option<i64>,
) -> ResponseResult<()> {
    let sender_id = msg.chat.id.0;

    // For /start and /chatid commands, always respond (helps with setup)
    match &cmd {
        Command::Start => {
            let welcome = format!(
                "Welcome to Screen Time Manager Bot!\n\n\
                 Your chat ID is: {}\n\n\
                 Use /help to see available commands.",
                sender_id
            );
            bot.send_message(msg.chat.id, welcome).await?;
            return Ok(());
        }
        Command::Chatid => {
            bot.send_message(msg.chat.id, format!("{} {}", i18n::t("tg.chatid.your_id"), sender_id)).await?;
            return Ok(());
        }
        _ => {}
    }

    // Authorization check for all other commands
    if let Some(admin_id) = admin_chat_id {
        if sender_id != admin_id {
            bot.send_message(msg.chat.id, i18n::t("tg.error.unauthorized")).await?;
            return Ok(());
        }
    } else {
        // No admin configured - reject all commands except /start and /chatid
        bot.send_message(msg.chat.id, i18n::t("tg.error.no_admin")).await?;
        return Ok(());
    }

    let response = match cmd {
        Command::Start => unreachable!(), // Handled above
        Command::Status => cmd_status(),
        Command::Time => cmd_time(),
        Command::Extend(mins) => cmd_extend(mins),
        Command::Reduce(mins) => cmd_reduce(mins),
        Command::Pause => cmd_pause(),
        Command::Resume => cmd_resume(),
        Command::History => cmd_history(),
        Command::Msg(text) => cmd_msg(&text),
        Command::Lock => cmd_lock(),
        Command::Stop => cmd_lock(),
        Command::Reset => cmd_reset(),
        Command::E30 => cmd_extend(30),
        Command::E60 => cmd_extend(60),
        Command::E120 => cmd_extend(120),
        Command::Chatid => unreachable!(), // Handled above
        Command::Help => Command::descriptions().to_string(),
    };

    bot.send_message(msg.chat.id, response).await?;
    Ok(())
}

// ============================================================================
// Command Implementations
// ============================================================================

fn cmd_status() -> String {
    let remaining = blocking::get_remaining_seconds();
    let paused = mini_overlay::is_paused();
    let idle_paused = mini_overlay::is_idle_paused();
    let pause_budget = mini_overlay::get_remaining_pause_budget();

    let mins = remaining / 60;
    let secs = remaining % 60;

    let status_emoji = if remaining <= 60 {
        "🔴"
    } else if remaining <= 300 {
        "🟠"
    } else {
        "🟢"
    };

    let pause_status = if paused {
        i18n::t("tg.status.yes")
    } else if idle_paused {
        i18n::t("tg.status.idle")
    } else {
        i18n::t("tg.status.no")
    };

    format!(
        "{}\n\
         ━━━━━━━━━━━━━━━━━━\n\
         {} {}: {}:{:02}\n\
         ⏸ {}: {}\n\
         🔋 {}: {} min",
        i18n::t("tg.status.header"),
        status_emoji,
        i18n::t("tg.status.remaining"),
        mins, secs,
        i18n::t("tg.status.paused"),
        pause_status,
        i18n::t("tg.status.pause_budget"),
        pause_budget / 60
    )
}

fn cmd_time() -> String {
    let remaining = blocking::get_remaining_seconds();
    let mins = remaining / 60;
    let secs = remaining % 60;

    let emoji = if remaining <= 60 {
        "🔴"
    } else if remaining <= 300 {
        "🟠"
    } else {
        "🟢"
    };

    format!("{} {}:{:02} remaining", emoji, mins, secs)
}

fn cmd_extend(minutes: i32) -> String {
    if minutes <= 0 {
        return i18n::t("tg.extend.specify_positive").to_string();
    }
    if minutes > 120 {
        return i18n::t("tg.extend.max_120").to_string();
    }

    blocking::extend_time(minutes);

    // Hide the blocking overlay if it's showing
    unsafe {
        blocking::hide_blocking_overlay();
    }

    // Get new remaining time
    let remaining = blocking::get_remaining_seconds();
    let new_mins = remaining / 60;
    let new_secs = remaining % 60;

    format!("✅ {} {} min\n{} {}:{:02}",
        i18n::t("tg.extend.success").replace("{}", ""),
        minutes,
        i18n::t("tg.status.remaining"),
        new_mins, new_secs)
}

fn cmd_reduce(minutes: i32) -> String {
    if minutes <= 0 {
        return i18n::t("tg.reduce.specify_positive").to_string();
    }
    if minutes > 120 {
        return i18n::t("tg.reduce.max_120").to_string();
    }

    let current = blocking::get_remaining_seconds();
    let reduction_seconds = minutes * 60;

    if reduction_seconds >= current {
        return format!("{} ({}:{:02})",
            i18n::t("tg.reduce.not_enough"),
            current / 60, current % 60);
    }

    blocking::reduce_time(minutes);

    // Get new remaining time
    let remaining = blocking::get_remaining_seconds();
    let new_mins = remaining / 60;
    let new_secs = remaining % 60;

    format!("⏬ {} {} min\n{} {}:{:02}",
        i18n::t("tg.reduce.success").replace("{}", ""),
        minutes,
        i18n::t("tg.status.remaining"),
        new_mins, new_secs)
}

fn cmd_pause() -> String {
    if mini_overlay::is_paused() {
        return format!("⏸ {}", i18n::t("tg.pause.already_paused"));
    }
    if mini_overlay::is_idle_paused() {
        return format!("⏸ {}", i18n::t("tg.pause.idle_paused"));
    }

    match mini_overlay::toggle_pause() {
        Ok(true) => format!("⏸ {}", i18n::t("tg.pause.success")),
        Ok(false) => i18n::t("tg.pause.failed").to_string(),
        Err(reason) => format!("{} {}", i18n::t("tg.pause.cannot"), format_pause_reason(reason)),
    }
}

fn cmd_resume() -> String {
    if mini_overlay::is_idle_paused() {
        return format!("▶️ {}", i18n::t("tg.resume.idle_auto"));
    }
    if !mini_overlay::is_paused() {
        return format!("▶️ {}", i18n::t("tg.resume.not_paused"));
    }

    match mini_overlay::toggle_pause() {
        Ok(false) => format!("▶️ {}", i18n::t("tg.resume.success")),
        Ok(true) => i18n::t("tg.resume.failed").to_string(),
        Err(reason) => format!("{} {}", i18n::t("tg.resume.cannot"), format_pause_reason(reason)),
    }
}

fn cmd_history() -> String {
    use std::sync::atomic::Ordering;

    let log = database::get_pause_log_today();
    let pause_used = database::get_pause_used_today();
    let pause_config = database::get_pause_config();
    let session_active = mini_overlay::SESSION_ACTIVE_SECONDS.load(Ordering::SeqCst);

    let mut response = format!("📊 {}\n━━━━━━━━━━━━━━━━━━\n", i18n::t("tg.history.header"));

    // Format uptime
    let hours = session_active / 3600;
    let minutes = (session_active % 3600) / 60;
    let seconds = session_active % 60;
    if hours > 0 {
        response.push_str(&format!("⏱ {} {}h {}m {}s\n", i18n::t("tg.history.uptime"), hours, minutes, seconds));
    } else {
        response.push_str(&format!("⏱ {} {}m {}s\n", i18n::t("tg.history.uptime"), minutes, seconds));
    }

    response.push_str(&format!(
        "⏸ {} {} / {} min\n\n",
        i18n::t("tg.history.pause_used"),
        pause_used / 60,
        pause_config.daily_budget_minutes
    ));

    if log.is_empty() {
        response.push_str(i18n::t("tg.history.no_events"));
    } else {
        response.push_str(&format!("{}:\n", i18n::t("stats.log")));
        for entry in log {
            response.push_str(&format!("• {}\n", entry));
        }
    }

    response
}

fn cmd_msg(text: &str) -> String {
    if text.is_empty() {
        return i18n::t("tg.msg.provide").to_string();
    }

    unsafe {
        overlay::show_overlay(text, 10);
    }

    format!("📢 {}: \"{}\"", i18n::t("tg.msg.shown"), text)
}

fn cmd_reset() -> String {
    let weekday = database::get_current_weekday();
    let daily_limit_minutes = database::get_daily_limit(weekday);
    let daily_limit_seconds = (daily_limit_minutes * 60) as i32;

    blocking::REMAINING_SECONDS.store(daily_limit_seconds, std::sync::atomic::Ordering::SeqCst);
    database::save_remaining_time(daily_limit_seconds);

    unsafe {
        mini_overlay::update_mini_overlay();
        // Hide the blocking overlay if it's showing
        blocking::hide_blocking_overlay();
    }

    format!(
        "🔄 {} ({} min)\n{} {}:{:02}",
        i18n::t("tg.reset.success"),
        daily_limit_minutes,
        i18n::t("tg.reset.remaining"),
        daily_limit_seconds / 60,
        daily_limit_seconds % 60
    )
}

fn cmd_lock() -> String {
    let message = database::get_blocking_message();

    unsafe {
        blocking::show_blocking_overlay(&message);
    }

    format!("🔒 {}", i18n::t("tg.lock.success"))
}

/// Format pause blocked reason for display
fn format_pause_reason(reason: mini_overlay::PauseBlockedReason) -> String {
    match reason {
        mini_overlay::PauseBlockedReason::Disabled => i18n::t("pause.disabled").to_string(),
        mini_overlay::PauseBlockedReason::BudgetExhausted => i18n::t("pause.budget_exhausted").to_string(),
        mini_overlay::PauseBlockedReason::CooldownActive { seconds_remaining } => {
            format!("{} ({}s)", i18n::t("pause.cooldown"), seconds_remaining)
        }
        mini_overlay::PauseBlockedReason::MinActiveTimeNotMet { seconds_remaining } => {
            format!("{} ({}s)", i18n::t("pause.min_active"), seconds_remaining)
        }
        mini_overlay::PauseBlockedReason::TimeTooLow => i18n::t("pause.time_too_low").to_string(),
    }
}
