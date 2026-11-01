# TaskBoot — Rust Style Guide (v2)

Keep the task, time and scheduling logic pure and unit-tested, keep every `unsafe` Win32 call behind a small safe wrapper at the edges, and build with a size-first release profile.

## Architecture rule

Logic in the middle, Win32 at the edges.

- `model`, `store`, `clock` and `scheduler` contain **no** Win32 calls and take the current time as a parameter. They are fully testable with `cargo test` on any OS.
- `tray`, `alert`, `window`, `autostart` and `win` are thin Win32 layers that call into the logic and render its results.
- `main` owns the single `App` state and the message loop; nothing else holds global mutable state.

## Module structure

```text
taskboot/
  Cargo.toml
  build.rs                embed icons + manifest
  .cargo/config.toml      static CRT
  assets/
    default_tasks.txt     created beside the exe on first run if missing
    app.ico               normal tray icon
    app_alert.ico         red tray icon (overdue state)
    app.manifest          PerMonitorV2 DPI, Common Controls v6
    app.rc
  src/
    main.rs               entry, single instance, App state, message loop, WndProc routing
    model.rs              Task, Settings, Status (Open/Done), Snooze
    clock.rs              Minute type, parse/format "YYYY-MM-DD HH:MM", now_local()
    store.rs              parse/serialize tasks.txt, atomic save, reload on mtime change
    scheduler.rs          pure: due events and next wake-up time
    alert.rs              fire an event: balloon, sound, tray icon swap
    tray.rs               Shell_NotifyIconW add/modify/delete, tray menu
    window.rs             task list window: ListView, editor, context menu
    autostart.rs          Run-key enable / disable / query
    win.rs                to_wide, Error, IDs and message constants
```

## Data and time conventions

- Time is `Minute(u64)`: minutes since 1970-01-01 in **local** time. All comparisons use it; no `chrono` (size).
- Convert date ↔ `Minute` with a pure days-from-civil function; only `clock::now_local()` touches Win32 (`GetLocalTime`).
- `tasks.txt` format, one task per line, `#` comments allowed:

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

- The parser never fails the whole file: a bad line is kept verbatim as `Task::Unparsed(String)` and written back unchanged.
- Snooze is persisted so it survives a restart; "last alerted" and "mute until" are in-memory only.

## Naming

- Standard Rust: `snake_case` functions and modules, `CamelCase` types, `SCREAMING_CASE` constants.
- IDs in `win.rs`: `WM_TRAY = WM_APP + 1`, `TIMER_MAIN = 1`, menu/command IDs from `1001` (`ID_OPEN`, `ID_ADD`, `ID_MUTE_1H`, `ID_AUTOSTART`, `ID_EXIT`, `ID_DONE`, `ID_SNOOZE_10M`, `ID_SNOOZE_1H`, `ID_SNOOZE_TOMORROW`, `ID_EDIT`, `ID_DELETE`).
- Events are an enum, not strings: `Event::Approaching(id)`, `Event::Overdue(id)`, `Event::DailySummary`, `Event::MissedSummary`.

## Error handling

Custom error type, not anyhow (size).

```rust
#[derive(Debug, Clone, Copy)]
pub enum Error { Win32(u32), Io(std::io::ErrorKind) }
pub type Result<T> = core::result::Result<T, Error>;
pub fn last_err() -> Error { Error::Win32(unsafe { GetLastError() }) }
```

- No `unwrap()`/`expect()` outside tests; avoid `format!` in common paths.
- A failed save shows one `MessageBoxW` and keeps the in-memory list; it never discards data.
- A missing `tasks.txt` is not an error: create it from the default.

## Windows API handling

- Each `unsafe` block sits in a wrapper of a few lines with a `// SAFETY:` comment.
- `to_wide(&str) -> Vec<u16>` adds the null terminator; keep the Vec alive for the call.
- **Timer:** one `SetTimer(hwnd, TIMER_MAIN, ms, None)` set to the next due event, capped at 60 min as a drift guard. On `WM_TIMER`: `KillTimer`, run the scheduler, fire events, reschedule.
- **Reschedule** also on: any task edit, `WM_POWERBROADCAST` + `PBT_APMRESUMEAUTOMATIC`, `WM_TIMECHANGE`, and file reload.
- **Alerts:** balloon via `NIF_INFO` (`NIIF_WARNING` for overdue); sound via `MessageBeep(MB_ICONWARNING)` (no winmm); overdue state swaps the tray icon with `NIM_MODIFY`.
- **Tray:** handle `NIN_BALLOONUSERCLICK`, `WM_LBUTTONUP`, `WM_RBUTTONUP`; `SetForegroundWindow` before `TrackPopupMenu`; `NIM_DELETE` in `WM_DESTROY`; re-add on `TaskbarCreated`.
- **Task window:** `InitCommonControlsEx(ICC_LISTVIEW_CLASSES | ICC_DATE_CLASSES)`; ListView in report mode with `LVS_EX_CHECKBOXES | LVS_EX_FULLROWSELECT`; the editor uses an Edit control plus a `DATETIMEPICK_CLASS` control with `DTS_SHOWNONE` for an optional deadline.
- **Registry:** `RegOpenKeyExW` / `RegSetValueExW` (`REG_SZ`, quoted exe path) / `RegDeleteValueW` / `RegQueryValueExW`; always `RegCloseKey`.
- **Atomic save:** write `tasks.txt.tmp`, flush, then `std::fs::rename` over `tasks.txt`.

## Build optimizations for size

```toml
# Cargo.toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
debug = false
```

```toml
# .cargo/config.toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

- `#![windows_subsystem = "windows"]` in `main.rs`.
- Enable only the `windows-sys` features used.
- Check with `cargo bloat --release --crates` after each feature step.
