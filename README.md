# TaskBoot

A lightweight Windows to-do list that lives in the system tray and alerts you when tasks are near their deadline, overdue, or still not done. One portable `.exe` (~260 KB), idles at 0% CPU and about 2 MB private memory.

## Use

1. Put `taskboot.exe` and `tasks.txt` in a folder you will keep.
2. Run `taskboot.exe` once. It registers itself in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` and starts at every login.
3. Left-click the tray icon to open the task list. Right-click for the menu: Open list, Add task, Mute alerts for 1 h, Auto-start on/off, Exit.

In the list: tick the checkbox to mark done, double-click to edit, right-click for Done / Snooze (10 min, 1 h, tomorrow 09:00) / Edit / Delete. Overdue tasks are red and sorted first.

## Alerts

| When | What |
| --- | --- |
| 15 min before a deadline | Notification with the time left |
| Deadline passed, not done | Notification, sound, red tray icon |
| Still overdue | Repeats every 30 min until done or snoozed |
| Tasks without deadline | One summary at login and at 16:00 |
| PC was off or asleep | One summary of everything missed at login or resume |

If Focus Assist hides a notification, the red tray icon still shows the overdue state.

## tasks.txt

Human-editable, picked up on the next timer tick or when the window opens:

```text
[settings]
lead_minutes=15
repeat_minutes=30
daily_summary=16:00

[ ] 2026-11-03 17:00 | Send weekly report
[ ] 2026-11-04 09:00 | Review PR #42 | snooze=2026-11-04 10:00
[x] 2026-11-01 12:00 | Book flight
[ ] | Clean up inbox
```

Lines TaskBoot cannot parse are kept exactly as written. Saves are atomic (temp file, then rename).

## Remove

Turn **Auto-start off** in the tray menu *before* deleting the exe, or the Run entry stays behind.

## Build

```text
cargo test
cargo build --release
```

Windows 10/11, x86_64, MSVC toolchain.
