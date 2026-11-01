# TaskBoot — Task List for Claude Code (v2)

Eight steps, each ending in a check Claude Code must pass before moving on. Give Claude Code `PRD.md` and `STYLE.md` as context, then work one step at a time. Steps 2–3 are pure logic and can be built and tested before any UI exists.

## Step 1 — Initialize the project

Files: `Cargo.toml`, `.cargo/config.toml`, `src/main.rs`, empty `src/{model,clock,store,scheduler,alert,tray,window,autostart,win}.rs`.

- `cargo new taskboot`; add the release profile and static-CRT config from `STYLE.md`.

```toml
[dependencies.windows-sys]
version = "0.59"
features = [
  "Win32_Foundation", "Win32_UI_WindowsAndMessaging", "Win32_UI_Shell",
  "Win32_UI_Controls", "Win32_System_Registry", "Win32_System_LibraryLoader",
  "Win32_System_Threading", "Win32_System_SystemInformation",
  "Win32_System_Power", "Win32_Graphics_Gdi",
]

[build-dependencies]
embed-resource = "3"
```

- [ ] Done when `cargo build --release` succeeds.

## Step 2 — Data model, time and storage

Files: `src/model.rs`, `src/clock.rs`, `src/store.rs`, `assets/default_tasks.txt`.

```rust
pub struct Minute(pub u64);                       // local minutes since 1970-01-01
pub enum Status { Open, Done }
pub struct Task { pub id: u32, pub title: String, pub deadline: Option<Minute>,
                  pub status: Status, pub snooze: Option<Minute> }
pub struct Settings { pub lead: u32, pub repeat: u32, pub daily_summary: (u8, u8) }
pub enum Line { Task(Task), Unparsed(String) }
```

- `clock`: `parse("2026-11-03 17:00") -> Option<Minute>`, `format(Minute)`, pure days-from-civil; `now_local()` via `GetLocalTime`.
- `store`: `parse(&str) -> (Settings, Vec<Line>)`, `serialize(..) -> String`, `load()`, `save()` (temp file + rename), `changed_on_disk() -> bool` (mtime).
- Strip a UTF-8 BOM; missing file → write the default and load it.
- [ ] Done when tests pass: round-trip keeps every line byte-identical, bad lines survive, BOM, missing deadline, snooze field, settings defaults, leap-year and month-end dates.

## Step 3 — Scheduler (pure)

Files: `src/scheduler.rs`.

```rust
pub struct AlertState { pub last_alerted: Vec<(u32, Minute)>, pub mute_until: Option<Minute>,
                        pub last_daily: Option<Minute> }

pub fn due_events(tasks: &[Task], s: &Settings, st: &AlertState, now: Minute) -> Vec<Event>;
pub fn next_wake(tasks: &[Task], s: &Settings, st: &AlertState, now: Minute) -> Minute;
```

- Rules from PRD A1–A5: approaching at `deadline - lead`; overdue at `deadline`, then every `repeat` minutes since last alert; skip done, snoozed (`snooze > now`) and muted; one `DailySummary` per day at the set time; `MissedSummary` once at startup when any open task is overdue (replaces individual overdue alerts for that tick).
- [ ] Done when table-driven tests pass for each rule, including snooze expiry, mute, done tasks, no-deadline tasks, and `next_wake` never returning a time in the past.

## Step 4 — Auto-launch

Files: `src/win.rs`, `src/autostart.rs`.

- `enable()`, `disable()`, `is_enabled()`, `toggle()` on value `TaskBoot` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, quoted exe path; `enable()` on first run if absent.
- [ ] Done when `reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v TaskBoot` shows the path after one run.

## Step 5 — Tray, timer and alerts

Files: `src/main.rs`, `src/tray.rs`, `src/alert.rs`, `build.rs`, `assets/app.rc`, `assets/app.manifest`, `assets/app.ico`, `assets/app_alert.ico`.

1. `main.rs`: single-instance mutex `Local\TaskBootSingleton`; hidden window; `App { settings, lines, alert_state }`; run the scheduler once at startup.
2. Scheduling loop:

```rust
fn tick(app: &mut App, hwnd: HWND) {
    if store::changed_on_disk() { app.reload(); }
    let now = clock::now_local();
    for ev in scheduler::due_events(&app.tasks(), &app.settings, &app.alert_state, now) {
        alert::fire(hwnd, &ev, app);          // balloon + MessageBeep, records last_alerted
    }
    tray::set_overdue(hwnd, app.any_overdue(now));
    let wait = scheduler::next_wake(&app.tasks(), &app.settings, &app.alert_state, now).0 - now.0;
    unsafe { SetTimer(hwnd, TIMER_MAIN, (wait.clamp(1, 60) * 60_000) as u32, None); }
}
```

3. Call `tick` on `WM_TIMER`, `PBT_APMRESUMEAUTOMATIC`, `WM_TIMECHANGE`, and after every edit.
4. Tray menu: Open list, Add task, Mute alerts 1 h, Auto-start on/off, Exit. Balloon click → open the list with the alerted task selected.
- [ ] Done when a task due in 2 minutes (lead set to 1) gives an approaching alert, then an overdue alert with sound and a red icon, then a repeat; marking it done in `tasks.txt` stops alerts and restores the icon.

## Step 6 — Task window

Files: `src/window.rs`.

- ListView (report, checkboxes): columns Task, Deadline. Sort overdue first (marked "OVERDUE"), then nearest deadline, then none, then done.
- Buttons: Add, Edit, Delete. Editor dialog: title Edit + `DATETIMEPICK_CLASS` with `DTS_SHOWNONE`.
- Checkbox toggle (`LVN_ITEMCHANGED`) → mark done/open, save, `tick`.
- Right-click menu: Done, Snooze 10 min / 1 h / tomorrow 09:00, Edit, Delete.
- [ ] Done when every action saves to `tasks.txt`, survives a restart, and immediately updates alerts and the tray icon.

## Step 7 — Optimize

- `cargo bloat --release --crates` and `-n 20`; remove `format!`, debug-print code, panicking indexing, unused features.
- [ ] Done when the .exe is < 1 MB, idle RAM < 10 MB, and CPU reads 0% between alerts (Task Manager → Details).

## Step 8 — Test and package

- [ ] `cargo test` passes.
- [ ] Sleep the PC past a deadline; on wake, one missed-alerts summary appears.
- [ ] Sign out and back in: app starts, summary shows overdue tasks.
- [ ] Edit `tasks.txt` in Notepad while running: changes are picked up on the next tick or window open.
- [ ] With Focus Assist on, the red tray icon still shows overdue state.
- [ ] Kill the process mid-save: `tasks.txt` is intact.
- [ ] Second launch blocked; Explorer restart restores the icon; Exit leaves no ghost icon.
- [ ] Zip `taskboot.exe`, sample `tasks.txt`, `README.md` into `dist/taskboot.zip`; README says to turn auto-start off before deleting the exe.
