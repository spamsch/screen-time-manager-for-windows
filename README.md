# Screen Time Manager

A Windows parental control application for managing children's screen time. The application runs quietly in the system tray and enforces daily time limits with customizable warnings. This is a solution that works on any Windows PC. It does not require Family Controls or Google Family Link. It just works.

I built this because I did not want to create a separate account for my kid. Also, he is sometimes on my Windows laptop and needs time control there too.

This solution is not safe for tech savvy kids as they can shut down the application through the task manager. But this can be solved in a future version.

---

## For Parents

### What Does It Do?

Screen Time Manager helps you control how much time your child spends on the computer each day. Once the daily limit is reached, the screen is blocked until a parent enters the passcode.

### Key Features

- **Daily Time Limits** - Set different time limits for each day of the week (e.g., 2 hours on weekdays, 4 hours on weekends)
- **Always-Visible Timer** - A small, unobtrusive timer display in the top-right corner shows remaining time at a glance
- **Pause Mode** - Children can pause the timer themselves (with configurable limits) for breaks, meals, or homework
- **Warning Notifications** - Configurable warnings before time runs out (e.g., "10 minutes remaining!" and "5 minutes remaining!")
- **Full-Screen Block** - When time is up, a full-screen overlay blocks the computer until the passcode is entered
- **Multi-Monitor Support** - Blocking overlay covers all connected monitors
- **Time Extensions** - Parents can grant extra time (+15, +30, or +60 minutes) by entering the passcode
- **Today's Stats** - View daily statistics including time used, time remaining, pause usage, and reset the timer if needed
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
- When paused, shows "II X:XX" (remaining pause time) in cyan

### Pause Mode

Pause mode gives children some autonomy to temporarily stop the timer without needing a parent's passcode. This is useful for meals, homework, or other activities where they need to step away.

#### How Children Use Pause

1. Right-click the tray icon
2. Click "Pause Timer" (shows remaining pause budget)
3. The timer stops and the mini overlay shows pause countdown
4. To resume, right-click and click "Resume Timer" (or wait for auto-resume)

#### Built-in Safeguards

Pause mode has multiple protections to prevent abuse:

| Protection | Default | Description |
|------------|---------|-------------|
| **Daily Budget** | 45 min | Total pause time allowed per day |
| **Max Duration** | 20 min | Single pause auto-resumes after this |
| **Cooldown** | 15 min | Must wait between pauses |
| **Min Active Time** | 10 min | Must use timer before first pause |
| **Low Time Block** | 1 min | Cannot pause with less than 1 minute remaining |

#### Pause Menu States

The tray menu shows why pause may be unavailable:
- **"Pause Timer (Xm left)"** - Available, shows remaining budget
- **"Resume Timer"** - Currently paused, click to resume
- **"Pause (Budget used)"** - Daily pause budget exhausted
- **"Pause (Xm cooldown)"** - Waiting for cooldown period
- **"Pause (wait Xm)"** - Need more active time first
- **"Pause (Time too low)"** - Less than 1 minute of screen time left
- **"Pause (Disabled)"** - Parent has disabled pause feature

#### Viewing Pause History

Parents can view pause usage in "Today's Stats...":
- Pause Used: X / 45 min
- Pause Remaining: time left in budget
- Pauses Today: count of pauses taken
- Log: timestamps of each pause

### Tips for Parents

- Choose a passcode your child cannot easily guess
- Set reasonable limits that balance screen time with other activities
- Use the warning messages to help children prepare to wrap up their activities
- The time tracking persists across restarts, so restarting the computer won't reset the timer
- Use "Today's Stats" to monitor usage and reset the timer when needed (e.g., for a fresh start)
- Review pause usage in "Today's Stats" to ensure pause mode isn't being abused
- If pause mode is being misused, you can disable it by setting `pause_enabled` to `0` in the database

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
| `pause_enabled` | Enable/disable pause feature | `1` (enabled) |
| `pause_daily_budget` | Total pause minutes per day | `45` |
| `pause_max_duration` | Max minutes per single pause | `20` |
| `pause_cooldown` | Minutes between pauses | `15` |
| `pause_min_active_time` | Min minutes before first pause | `10` |
| `pause_used_YYYY-MM-DD` | Pause seconds used for date | (dynamic) |
| `pause_log_YYYY-MM-DD` | Comma-separated pause log | (dynamic) |

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

### Multi-Monitor Support

When the blocking overlay is triggered, all connected monitors are blanked:
- The primary monitor displays the full blocking UI with passcode entry
- Secondary monitors display a simple "Screen Locked" message
- All overlays are shown/hidden together

### Autostart Configuration

To have Screen Time Manager start automatically when Windows boots, you can use one of the following methods:

#### Method 1: Startup Folder (Recommended for single user)

1. Press `Win + R` to open the Run dialog
2. Type `shell:startup` and press Enter
3. Copy `screen-time-manager.exe` to this folder, or create a shortcut:
   - Right-click in the folder and select **New > Shortcut**
   - Browse to the location of `screen-time-manager.exe`
   - Click **Next** and give it a name like "Screen Time Manager"

#### Method 2: Registry (All users)

Run this command in an elevated Command Prompt or PowerShell:

```cmd
reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" /v "ScreenTimeManager" /t REG_SZ /d "C:\Path\To\screen-time-manager.exe" /f
```

Replace `C:\Path\To\screen-time-manager.exe` with the actual path to the executable.

To remove:
```cmd
reg delete "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" /v "ScreenTimeManager" /f
```

#### Method 3: Task Scheduler (Most control)

1. Open **Task Scheduler** (search for it in the Start menu)
2. Click **Create Task** (not "Create Basic Task")
3. **General tab**:
   - Name: "Screen Time Manager"
   - Select "Run whether user is logged on or not"
   - Check "Run with highest privileges"
4. **Triggers tab**:
   - Click **New**
   - Begin the task: "At log on"
   - Select "Any user" or a specific user
5. **Actions tab**:
   - Click **New**
   - Action: "Start a program"
   - Browse to `screen-time-manager.exe`
6. **Conditions tab**:
   - Uncheck "Start the task only if the computer is on AC power"
7. Click **OK** and enter admin credentials if prompted

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

### Future Improvements / TODO

This section lists known limitations and potential enhancements for future development.

#### Security Hardening (High Priority)

- [ ] **Database encryption** - Encrypt the SQLite database or at least the passcode field to prevent tampering
- [ ] **Passcode hashing** - Store passcode as a salted hash instead of plain text
- [ ] **Database file protection** - Set restrictive ACLs on the database file to prevent non-admin users from modifying it
- [ ] **Anti-tampering detection** - Detect if the database has been modified externally and take action (e.g., lock the screen)
- [ ] **Process protection** - Prevent the process from being killed via Task Manager by non-admin users (requires running as SYSTEM or using a kernel driver)
- [ ] **Prevent system time manipulation** - Detect and handle attempts to change the system clock to bypass time limits

#### Installation & Deployment

- [ ] **Windows Service mode** - Run as a Windows Service instead of a user-space application for better protection and earlier startup
- [ ] **MSI/MSIX installer** - Create a proper installer with:
  - Automatic service registration
  - Autostart configuration
  - Uninstaller
  - Upgrade support
- [ ] **Group Policy support** - Allow deployment and configuration via Active Directory Group Policy

#### Features

- [ ] **Historical usage statistics** - Track and display weekly/monthly usage trends with graphs
- [ ] **Multiple user profiles** - Support different limits for different Windows user accounts
- [ ] **Remote management** - Web interface or mobile app for parents to monitor and adjust settings remotely
- [ ] **Application-specific limits** - Set time limits for specific applications (e.g., games) separately from total screen time
- [ ] **Break reminders** - Configurable reminders to take breaks (e.g., "Take a 5-minute break every hour")
- [ ] **Schedule-based limits** - Allow different limits based on time of day (e.g., no screen time after 9 PM)
- [ ] **Reward system** - Allow parents to grant bonus time for completing tasks/chores
- [ ] **Grace period** - Configurable grace period after time expires to save work
- [ ] **Activity logging** - Log when the computer was used, when blocks occurred, when extensions were granted
- [ ] **Export/Import settings** - Backup and restore configuration

#### User Interface

- [ ] **Modern UI** - Consider using a UI framework (WinUI 3, egui) for a more modern appearance
- [ ] **Localization** - Support multiple languages
- [ ] **Custom themes** - Allow customization of overlay colors and fonts
- [ ] **Accessibility** - Ensure screen reader compatibility and keyboard navigation
- [ ] **High DPI support** - Proper scaling on high-resolution displays

#### Technical Improvements

- [ ] **Logging** - Add configurable logging for debugging and auditing
- [ ] **Error handling** - More robust error handling with user-friendly error messages
- [ ] **Configuration file** - Support an alternative configuration file in addition to the database
- [ ] **Portable mode** - Option to store data next to the executable instead of in AppData
- [ ] **Update mechanism** - Automatic update checking and installation
- [ ] **Crash recovery** - Automatically restart if the application crashes
- [ ] **Memory optimization** - Profile and optimize memory usage for long-running sessions

#### Testing

- [ ] **Unit tests** - Add comprehensive unit tests for core logic
- [ ] **Integration tests** - Test database operations and window management
- [ ] **Fuzzing** - Fuzz test the passcode entry and settings dialogs
- [ ] **CI/CD pipeline** - Automated building and testing on commits
