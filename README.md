# Screen Time Manager

A Windows parental control application for managing children's screen time. The application runs quietly in the system tray and enforces daily time limits with customizable warnings.

---

## For Parents

### What Does It Do?

Screen Time Manager helps you control how much time your child spends on the computer each day. Once the daily limit is reached, the screen is blocked until a parent enters the passcode.

### Key Features

- **Daily Time Limits** - Set different time limits for each day of the week (e.g., 2 hours on weekdays, 4 hours on weekends)
- **Always-Visible Timer** - A small, unobtrusive timer display in the top-right corner shows remaining time at a glance
- **Warning Notifications** - Configurable warnings before time runs out (e.g., "10 minutes remaining!" and "5 minutes remaining!")
- **Full-Screen Block** - When time is up, a full-screen overlay blocks the computer until the passcode is entered
- **Time Extensions** - Parents can grant extra time (+15, +30, or +60 minutes) by entering the passcode
- **Today's Stats** - View daily statistics including time used, time remaining, and reset the timer if needed
- **Passcode Protection** - All settings and unlocking require a 4-digit passcode (changeable in settings)
- **Persistent Tracking** - Time tracking continues even if the computer restarts

### How to Use

1. **Start the Application** - Run the application. It will appear as an icon in the system tray (bottom-right of the screen, near the clock). A small timer will also appear in the top-right corner showing remaining time.

2. **Access Settings** - Right-click the tray icon and select "Settings...". Enter your passcode (default: `0000`).

3. **Configure Time Limits** - Set the daily limit in minutes for each day:
   - Weekdays: Recommended 60-120 minutes
   - Weekends: Recommended 120-240 minutes

4. **Configure Warnings** - Set when warnings should appear:
   - First warning: e.g., 10 minutes before limit
   - Second warning: e.g., 5 minutes before limit
   - Customize the warning messages

5. **Change the Passcode** - In Settings, scroll to the "Change Passcode" section. Enter your current passcode, then enter and confirm the new 4-digit passcode.

6. **View Today's Stats** - Right-click the tray icon and select "Today's Stats..." to see:
   - Current day and configured daily limit
   - Time used today
   - Time remaining (color-coded: green > 5min, orange <= 5min, red <= 1min)
   - Option to reset the timer to the full daily limit

### When Time Runs Out

When the daily limit is reached:

1. A full-screen "Time's Up!" overlay appears
2. The remaining time (if any extensions were granted) is displayed
3. Your child can request more time using the extension buttons
4. **All actions require the parent's passcode**

### Granting Extra Time

From the blocking screen, you can:
- Enter your passcode and click **+15 min**, **+30 min**, or **+60 min** to grant extra time
- Enter your passcode and click **Unlock** to completely unlock the computer

### The Mini Timer

The small timer overlay in the top-right corner:
- Shows remaining time in a compact format (e.g., "1:30:45" or "30:45")
- Changes color as time runs low:
  - **White**: More than 5 minutes remaining
  - **Orange**: 5 minutes or less
  - **Red**: 1 minute or less
- Is click-through (doesn't interfere with using the computer)
- Automatically hides when the blocking screen appears

### Tips for Parents

- Choose a passcode your child cannot easily guess
- Set reasonable limits that balance screen time with other activities
- Use the warning messages to help children prepare to wrap up their activities
- The time tracking persists across restarts, so restarting the computer won't reset the timer
- Use "Today's Stats" to monitor usage and reset the timer when needed (e.g., for a fresh start)

---

## Technical Details

### System Requirements

- Windows 10 or later (64-bit)
- No additional runtime dependencies

### Architecture

The application is built in Rust using the Windows API directly for maximum compatibility and minimal resource usage.

#### Module Structure

```
src/
├── main.rs          - Entry point, single instance check, message loop
├── constants.rs     - Menu IDs, colors, configuration constants
├── database.rs      - SQLite database for settings and time tracking
├── overlay.rs       - Warning banner overlay (click-through, auto-hide)
├── blocking.rs      - Full-screen blocking overlay with passcode entry
├── mini_overlay.rs  - Always-visible mini timer in top-right corner
├── dialogs.rs       - Passcode verification, settings, and stats dialogs
└── tray.rs          - System tray icon and context menu
```

### Data Storage

Settings and time tracking data are stored in a SQLite database located at:
```
%LOCALAPPDATA%\.screen-time-manager\data.db
```

The directory is marked as hidden.

#### Database Schema

```sql
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
)
```

**Settings Keys:**
| Key | Description | Default |
|-----|-------------|---------|
| `passcode` | 4-digit unlock code | `0000` |
| `limit_monday` ... `limit_sunday` | Daily limits in minutes | 120-240 |
| `warning1_minutes` | First warning threshold | 10 |
| `warning1_message` | First warning text | "10 minutes remaining!" |
| `warning2_minutes` | Second warning threshold | 5 |
| `warning2_message` | Second warning text | "5 minutes remaining!" |
| `blocking_message` | Message shown on block screen | "Your screen time limit has been reached." |
| `remaining_time_YYYY-MM-DD` | Remaining seconds for date | (dynamic) |

### Window Styles

#### Warning Overlay
- `WS_EX_TOPMOST` - Always on top
- `WS_EX_LAYERED` - Supports transparency (alpha: 230/255)
- `WS_EX_TRANSPARENT` - Click-through
- `WS_EX_TOOLWINDOW` - Hidden from taskbar

#### Blocking Overlay
- `WS_EX_TOPMOST` - Always on top
- `WS_EX_TOOLWINDOW` - Hidden from taskbar
- Full-screen coverage
- Timer reasserts topmost position every 500ms

#### Mini Timer Overlay
- `WS_EX_TOPMOST` - Always on top
- `WS_EX_LAYERED` - Supports transparency (alpha: 200/255)
- `WS_EX_TRANSPARENT` - Click-through
- `WS_EX_TOOLWINDOW` - Hidden from taskbar
- Positioned in top-right corner (140x36 pixels)

### Single Instance

The application uses a named mutex to ensure only one instance runs:
```
Global\ScreenTimeManager_SingleInstance_7F3A9B2E
```

### Building from Source

#### Prerequisites
- Rust toolchain (rustup.rs)
- Windows SDK (for resource compilation)

#### Build Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

The release binary will be at `target/release/screen-time-manager.exe`.

#### Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `windows` | 0.58 | Windows API bindings |
| `rusqlite` | 0.32 | SQLite database (bundled) |
| `dirs` | 5.0 | Platform-specific directories |

### Security Considerations

- The passcode is stored in plain text in the SQLite database
- The database file is hidden but not encrypted
- A determined user with admin access could modify the database directly
- The application does not prevent Task Manager from being opened (this would require kernel-level access)

### Limitations

- Does not track time across multiple user accounts
- Cannot prevent a user with administrator privileges from terminating the process
- Time tracking is based on system clock (changing system time could affect tracking)

### Future Improvements

Potential enhancements that could be added:
- Historical usage statistics and reporting (weekly/monthly trends)
- Multiple user profiles
- Remote management via web interface
- Application-specific time limits
- Break reminders (e.g., "Take a 5-minute break every hour")
- Schedule-based automatic enabling/disabling
