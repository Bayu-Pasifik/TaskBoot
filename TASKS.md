# TaskBoot — Task List for Claude Code

Six steps, each ending in a check Claude Code can verify before moving on. Give Claude Code `PRD.md` and `STYLE.md` as context, then work one step at a time.

## Step 1 — Initialize the project

Files: `Cargo.toml`, `.cargo/config.toml`, `src/main.rs`, empty `src/{tasks,autostart,tray,window,win}.rs`.

- Run `cargo new taskboot`.
- Add the release profile and static-CRT config from `STYLE.md`.
- Add the dependency:

```toml
[dependencies.windows-sys]
version = "0.59"
features = [
  "Win32_Foundation", "Win32_UI_WindowsAndMessaging", "Win32_UI_Shell",
  "Win32_System_Registry", "Win32_System_LibraryLoader",
  "Win32_System_Threading", "Win32_Graphics_Gdi",
]

[build-dependencies]
embed-resource = "3"
```

- Put `#![windows_subsystem = "windows"]` and `mod` declarations in `main.rs`.
- [ ] Done when `cargo build --release` succeeds.

## Step 2 — Core reminder logic

Files: `assets/default_tasks.txt`, `src/tasks.rs`.

```rust
const DEFAULT: &str = include_str!("../assets/default_tasks.txt");

pub fn tasks_path() -> Option<std::path::PathBuf> {
    std::env::current_exe().ok().map(|p| p.with_file_name("tasks.txt"))
}

pub fn load() -> Vec<String> {
    let text = tasks_path().and_then(|p| std::fs::read_to_string(p).ok());
    parse(text.as_deref().unwrap_or(DEFAULT))
}

fn parse(s: &str) -> Vec<String> {
    s.lines().map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from).collect()
}
```

- Strip a UTF-8 BOM if present (Notepad may add one).
- [ ] Done when `#[cfg(test)]` tests pass for: blank lines, comments, BOM, empty file.

## Step 3 — Auto-launch via registry

Files: `src/win.rs` (`to_wide`, `Error`), `src/autostart.rs`.

- `enable()`, `disable()`, `is_enabled()`, `toggle()` on value name `TaskBoot` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
- Store the exe path in quotes, e.g. `"C:\path\taskboot.exe"`.
- In `main.rs`, call `enable()` on first run if the value is absent, so running the exe once installs it.
- [ ] Done when `reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v TaskBoot` shows the quoted path after one run.

## Step 4 — UI: tray, balloon, task window

Files: `src/main.rs`, `src/tray.rs`, `src/window.rs`, `build.rs`, `assets/app.rc`, `assets/app.manifest`, `assets/app.ico`.

1. `main.rs`: `CreateMutexW(null, 0, L"Local\\TaskBootSingleton")`; exit if `GetLastError() == ERROR_ALREADY_EXISTS`. Register a window class, create a hidden top-level window that owns the tray icon.
2. `tray.rs`: `add(hwnd)`, `balloon(hwnd, title, body)` (truncate body to 255 UTF-16 units, the `szInfo` limit), `remove(hwnd)`, `show_menu(hwnd)`.
3. `window.rs`: on show, create a 360×420 `WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU` window with a `LISTBOX` child (`LB_ADDSTRING` per task) and a Dismiss button. Set the font with `WM_SETFONT` + `GetStockObject(DEFAULT_GUI_FONT)`.
4. WndProc routing:

```rust
match msg {
    WM_TRAY => match (lparam as u32) & 0xFFFF {
        NIN_BALLOONUSERCLICK | WM_LBUTTONUP => window::show(hwnd),
        WM_RBUTTONUP => tray::show_menu(hwnd),
        _ => {}
    },
    WM_COMMAND => match (wparam & 0xFFFF) as u16 {
        ID_SHOW => window::show(hwnd),
        ID_EDIT => open_in_notepad(),
        ID_AUTOSTART => autostart::toggle(),
        ID_EXIT => unsafe { DestroyWindow(hwnd); },
        _ => {}
    },
    WM_DESTROY => { tray::remove(hwnd); unsafe { PostQuitMessage(0) } }
    _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
}
0
```

5. `build.rs`: `embed_resource::compile("assets/app.rc", embed_resource::NONE);` with the icon and a manifest declaring PerMonitorV2 DPI awareness and Common Controls v6.

- [ ] Done when launching shows the tray icon, the balloon appears, clicking it opens the list, and Exit removes the icon.

## Step 5 — Optimize size and resources

Files: `Cargo.toml` (feature trimming), any module flagged by cargo-bloat.

- `cargo install cargo-bloat`, then `cargo bloat --release --crates` and `cargo bloat --release -n 20`.
- Remove what dominates: usually `format!`, `{:?}` used in output, panicking slice indexing, unused `windows-sys` features.
- Optional: `upx --best`. Skip it if antivirus flags the packed binary.
- [ ] Done when the .exe is under 1 MB, idle working set is under 10 MB (Task Manager → Details), and CPU reads 0% after the balloon.

## Step 6 — Test and package

Files: `README.md`, `dist/` (created by the packaging step).

- [ ] `cargo test` passes.
- [ ] First run adds the Run entry; sign out and back in shows the reminder.
- [ ] Missing `tasks.txt` falls back to defaults; editing it updates the next launch.
- [ ] A second launch is blocked by the mutex.
- [ ] Auto-start toggle adds and removes the registry value.
- [ ] Restarting Explorer brings the tray icon back.
- [ ] Exit leaves no ghost tray icon.
- [ ] Zip `taskboot.exe` + sample `tasks.txt` + `README.md` into `dist/taskboot.zip`. README notes: turn auto-start off before deleting the exe.
