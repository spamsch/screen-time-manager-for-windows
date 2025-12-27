//! Telegram bot module for Screen Time Manager
//! Provides remote monitoring and control via Telegram commands

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use teloxide::prelude::*;
use teloxide::error_handlers::LoggingErrorHandler;
use teloxide::utils::command::BotCommands;

use crate::blocking;
use crate::database;
use crate::mini_overlay;

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
    #[command(description = "Pause the timer")]
    Pause,
    #[command(description = "Resume the timer")]
    Resume,
    #[command(description = "Show today's pause activity")]
    History,
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
    eprintln!("[Telegram] Starting bot thread...");
    eprintln!("[Telegram] Token: {}", token);
    eprintln!("[Telegram] Admin chat ID: {:?}", admin_chat_id);

    // Store admin chat ID for notifications
    if let Some(id) = admin_chat_id {
        let _ = ADMIN_CHAT_ID.set(id);
    }

    std::thread::spawn(move || {
        eprintln!("[Telegram] Bot thread started");
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            run_bot(token, admin_chat_id).await;
        });
        eprintln!("[Telegram] Bot thread ended");
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
                    let _ = bot.send_message(ChatId(chat_id), "Screen Time Manager is shutting down").await;
                });
            }
        });
        // Give a moment for the message to send
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Main bot loop
async fn run_bot(token: String, admin_chat_id: Option<i64>) {
    eprintln!("[Telegram] Initializing bot...");
    let bot = Bot::new(&token);

    // Store bot instance for notifications
    let _ = BOT_INSTANCE.set(bot.clone());

    // Send startup notification
    if let Some(chat_id) = admin_chat_id {
        eprintln!("[Telegram] Sending startup notification to chat {}", chat_id);
        match bot.send_message(ChatId(chat_id), "Screen Time Manager started").await {
            Ok(_) => eprintln!("[Telegram] Startup notification sent successfully"),
            Err(e) => eprintln!("[Telegram] Failed to send startup notification: {}", e),
        }
    }

    eprintln!("[Telegram] Setting up command handlers...");

    // Command handler
    let command_handler = Update::filter_message()
        .filter_command::<Command>()
        .endpoint(move |bot: Bot, msg: Message, cmd: Command| {
            handle_command(bot, msg, cmd, admin_chat_id)
        });

    // Fallback handler for unrecognized messages (helps with debugging)
    let fallback_handler = Update::filter_message()
        .endpoint(|bot: Bot, msg: Message| async move {
            // Only respond to text messages that look like commands
            if let Some(text) = msg.text() {
                if text.starts_with('/') {
                    bot.send_message(
                        msg.chat.id,
                        format!("Unknown command: {}\nUse /help to see available commands.", text)
                    ).await?;
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
    eprintln!("[Telegram] Bot is now running and listening for commands...");
    dispatcher.dispatch().await;
    eprintln!("[Telegram] Dispatcher stopped");
}

/// Handle incoming commands
async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    admin_chat_id: Option<i64>,
) -> ResponseResult<()> {
    let sender_id = msg.chat.id.0;
    eprintln!("[Telegram] Received command {:?} from chat {}", cmd, sender_id);

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
            bot.send_message(msg.chat.id, format!("Your chat ID is: {}", sender_id)).await?;
            return Ok(());
        }
        _ => {}
    }

    // Authorization check for all other commands
    if let Some(admin_id) = admin_chat_id {
        if sender_id != admin_id {
            bot.send_message(msg.chat.id, "Unauthorized. This bot is configured for a specific user.").await?;
            return Ok(());
        }
    } else {
        // No admin configured - reject all commands except /start and /chatid
        bot.send_message(msg.chat.id, "No admin configured. Please set your chat ID in settings.").await?;
        return Ok(());
    }

    let response = match cmd {
        Command::Start => unreachable!(), // Handled above
        Command::Status => cmd_status(),
        Command::Time => cmd_time(),
        Command::Extend(mins) => cmd_extend(mins),
        Command::Pause => cmd_pause(),
        Command::Resume => cmd_resume(),
        Command::History => cmd_history(),
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

    format!(
        "Screen Time Status\n\
         ━━━━━━━━━━━━━━━━━━\n\
         {} Remaining: {}:{:02}\n\
         ⏸ Paused: {}\n\
         🔋 Pause budget: {} min",
        status_emoji,
        mins, secs,
        if paused { "Yes" } else { "No" },
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
        return "Please specify a positive number of minutes".to_string();
    }
    if minutes > 120 {
        return "Maximum extension is 120 minutes".to_string();
    }

    blocking::extend_time(minutes);

    // Get new remaining time
    let remaining = blocking::get_remaining_seconds();
    let new_mins = remaining / 60;
    let new_secs = remaining % 60;

    format!("✅ Extended by {} minutes\nNew remaining: {}:{:02}", minutes, new_mins, new_secs)
}

fn cmd_pause() -> String {
    if mini_overlay::is_paused() {
        return "⏸ Timer is already paused. Use /resume to continue.".to_string();
    }

    match mini_overlay::toggle_pause() {
        Ok(true) => "⏸ Timer paused".to_string(),
        Ok(false) => "Timer was not paused (unexpected state)".to_string(),
        Err(reason) => format!("Cannot pause: {}", format_pause_reason(reason)),
    }
}

fn cmd_resume() -> String {
    if !mini_overlay::is_paused() {
        return "▶️ Timer is not paused".to_string();
    }

    match mini_overlay::toggle_pause() {
        Ok(false) => "▶️ Timer resumed".to_string(),
        Ok(true) => "Timer is still paused (unexpected state)".to_string(),
        Err(reason) => format!("Cannot resume: {}", format_pause_reason(reason)),
    }
}

fn cmd_history() -> String {
    let log = database::get_pause_log_today();
    let pause_used = database::get_pause_used_today();
    let pause_config = database::get_pause_config();

    let mut response = String::from("📊 Today's Activity\n━━━━━━━━━━━━━━━━━━\n");

    response.push_str(&format!(
        "Pause used: {} / {} min\n\n",
        pause_used / 60,
        pause_config.daily_budget_minutes
    ));

    if log.is_empty() {
        response.push_str("No pause events today");
    } else {
        response.push_str("Pause log:\n");
        for entry in log {
            response.push_str(&format!("• {}\n", entry));
        }
    }

    response
}

/// Format pause blocked reason for display
fn format_pause_reason(reason: mini_overlay::PauseBlockedReason) -> String {
    match reason {
        mini_overlay::PauseBlockedReason::Disabled => "Pause feature is disabled".to_string(),
        mini_overlay::PauseBlockedReason::BudgetExhausted => "Daily pause budget exhausted".to_string(),
        mini_overlay::PauseBlockedReason::CooldownActive { seconds_remaining } => {
            format!("Cooldown active ({} seconds remaining)", seconds_remaining)
        }
        mini_overlay::PauseBlockedReason::MinActiveTimeNotMet { seconds_remaining } => {
            format!("Need {} more seconds of active time", seconds_remaining)
        }
        mini_overlay::PauseBlockedReason::TimeTooLow => "Time is too low to pause".to_string(),
    }
}
