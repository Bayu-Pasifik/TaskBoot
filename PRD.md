# TaskBoot — Product Requirements

TaskBoot is a single portable .exe (target 150–300 KB, hard limit 1 MB) that shows your job tasks at every Windows login and then idles at 0% CPU and under 10 MB RAM.

## Purpose and user

One user: you, on your own Windows 10/11 PC. The goal is to see today's tasks the moment you log in, without a heavyweight to-do app slowing startup or sitting in memory.

## Core features

| ID | Feature | Behavior |
| --- | --- | --- |
| F1 | Auto-start | Registers under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` on first run. Per-user, no admin rights. |
| F2 | Login reminder | Tray icon plus a balloon notification (rendered as a toast on 10/11): title "N tasks today", body = first task. |
| F3 | Task window | Click balloon or tray icon → small fixed window (360×420 px) listing all tasks, with a Dismiss button. |
| F4 | Task source | `tasks.txt` next to the .exe, one task per line, `#` lines ignored. Missing file → compiled-in default list. |
| F5 | Tray menu | Show Tasks, Edit Tasks (opens tasks.txt in Notepad), Auto-start on/off, Exit. |
| F6 | Single instance | Named mutex; a second launch exits silently. |

Out of scope for v1: task completion tracking, scheduling by date, sync, multiple users.

## Non-functional requirements

| Requirement | Target | How it is met |
| --- | --- | --- |
| Binary size | < 1 MB (aim 150–300 KB) | Raw Win32, size-optimized release profile, stripped |
| Idle memory | < 10 MB working set (expect 2–4 MB) | No framework runtime, one hidden window, small allocations |
| Idle CPU | 0% | Blocking `GetMessageW` loop, no timers or polling |
| Startup | Tray + balloon < 200 ms after launch | No network, one small file read |
| Dependencies | Rust std + `windows-sys` only | `embed-resource` allowed as a build-dependency (not shipped) |

## GUI library decision

Use raw Win32 through `windows-sys`, not `winit`. winit only creates windows: no tray icon, notifications or native controls, so Win32 calls are needed anyway, and it adds roughly 300 KB+. `windows-sys` is zero-cost FFI with no runtime. If a richer UI is wanted later, `native-windows-gui` is the next step up, at a clear size cost.

## Constraints

- Windows 10 and 11, x86_64, MSVC toolchain.
- No installer: one .exe, optional `tasks.txt` beside it.
- CRT statically linked, so no VC++ redistributable is needed.
- Runs at user login (the Run key), which is "on boot" for a desktop user.

## Assumptions and open decisions

- **"On boot" means at login.** The Run key fires when you sign in. A reminder before login would need a Windows service, which cannot show UI on your desktop.
- **GUI = raw Win32 via `windows-sys`.** Chosen for size and because tray and notifications are Win32 anyway. Revisit only if the UI grows beyond a list and a few buttons.
- **Balloon, not a full WinRT toast.** `NIF_INFO` balloons render as toasts on 10/11 with no extra dependency. Rich toasts (buttons, images) would need the `windows` crate and an AppUserModelID, adding size.
- **`windows-sys` version 0.59.** Check the latest on crates.io at Step 1; the feature names match the 0.5x series.
- **Tasks are plain text, read once at launch.** No live reload; edits apply on the next login or manual relaunch.
- **x86_64 MSVC only.** ARM64 Windows would need a second build target.
