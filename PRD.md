# TaskBoot — Product Requirements (v2)

## 1. Product Overview

TaskBoot is a lightweight Windows to-do list that runs from the system tray and alerts you when tasks are near their deadline, past it, or still not done. It is a single portable .exe (under 1 MB) that starts at login and stays resident with near-zero resource use.

## 2. User Persona and Goal

- **User:** One knowledge worker (you) on a personal Windows 10/11 PC, juggling several job tasks with deadlines.
- **Goal:** Never miss a deadline or forget an unfinished task, without opening a heavy app or checking a list manually.

## 3. Core Features

### 3.1 Task list

| ID | Feature | Behavior |
| --- | --- | --- |
| F1 | Add / edit / delete | Small window: title (required), deadline date + time (optional). |
| F2 | Mark done | Checkbox; done tasks move to the bottom and stop alerting. |
| F3 | Views | Overdue (red) first, then by nearest deadline, then no deadline. |
| F4 | Storage | `tasks.txt` beside the .exe, human-editable, e.g. `[ ] 2026-11-03 17:00 \| Send report`. |

### 3.2 Alarm and warning system (priority feature)

| ID | Trigger | Alert |
| --- | --- | --- |
| A1 | Deadline approaching (default 15 min before) | Notification: task title + time left. |
| A2 | Deadline passed, not done | Notification **and** sound; tray icon turns red. |
| A3 | Still overdue | Repeats every 30 min (configurable) until done or snoozed. |
| A4 | Open tasks without deadline | One summary reminder at login and at 16:00 daily. |
| A5 | Missed while PC was off/asleep | All missed alerts shown as one summary on login or resume. |

Alert actions: **Mark done**, **Snooze** (10 min / 1 h / tomorrow), **Open list**. Clicking the notification opens the task window.

### 3.3 Supporting features

- Auto-start at login (per-user Run key), with toggle in the tray menu.
- Tray menu: Open list, Add task, Mute alerts for 1 h, Auto-start on/off, Exit.
- Single instance via named mutex.

## 4. Non-Functional Requirements

| Area | Requirement |
| --- | --- |
| Reliability | Alert fires within 60 s of its scheduled time while the app runs; no alert lost across sleep, restart or crash. |
| Data safety | Saves are atomic (write temp file, then rename); a malformed line is skipped, never deletes data. |
| Performance | 0% CPU between alerts: one OS timer set to the next due event, no polling. Idle RAM < 10 MB. |
| Size | Single .exe < 1 MB, no installer, no runtime dependencies. |
| Platform | Windows 10/11 x86_64, Rust + `windows-sys` (raw Win32). |
| Visibility | If Focus Assist suppresses a notification, the red tray icon and login summary still show it. |

## 5. Assumptions and Constraints

**Assumptions (not stated in the original):**

- One user, one PC; no sync, accounts or mobile app.
- "Incomplete" means not marked done; tasks without a deadline get only the daily summary (A4), not repeated alarms.
- Deadlines use local time; no recurring tasks or priorities in v1.
- Alert defaults (15 min, 30 min, 16:00) are editable in a `[settings]` block in `tasks.txt`.
- Notifications use Win32 balloons (shown as toasts on 10/11); action buttons appear in the task window if the balloon cannot host them.

**Constraints:**

- The app must stay running in the tray to alert; it cannot alert while the PC is off (covered by A5).
- No network access, telemetry or cloud storage.
- Must remain under 1 MB and 10 MB RAM.
