# TaskBoot — Rust Style Guide

Keep every `unsafe` Win32 call behind a small safe wrapper, use a tiny custom error type instead of anyhow, and build with a size-first release profile.

## Module structure

```text
taskboot/
  Cargo.toml
  build.rs              embed icon + manifest resource
  .cargo/config.toml    static CRT
  assets/
    default_tasks.txt   compiled-in fallback list
    app.ico
    app.manifest        DPI awareness, Common Controls v6
    app.rc
  src/
    main.rs             entry, single-instance check, message loop
    tasks.rs            load/parse tasks.txt, embedded fallback
    autostart.rs        registry enable / disable / query
    tray.rs             Shell_NotifyIconW add/modify/delete, context menu
    window.rs           task list window, WndProc
    win.rs              helpers: wide strings, Error type, constants
```

## Naming

- Standard Rust: `snake_case` functions and modules, `CamelCase` types, `SCREAMING_CASE` constants.
- Win32 message and command IDs as constants in `win.rs`: `WM_TRAY = WM_APP + 1`, `ID_SHOW = 1001`, `ID_EDIT = 1002`, `ID_AUTOSTART = 1003`, `ID_EXIT = 1004`.
- Public functions are named for the action, called through the module: `tray::add`, `tray::balloon`, `autostart::enable`.

## Error handling

Use a custom error type, not anyhow. anyhow brings backtrace and formatting machinery that costs size for no benefit in an app this small.

```rust
#[derive(Debug, Clone, Copy)]
pub enum Error { Win32(u32), Io(std::io::ErrorKind) }
pub type Result<T> = core::result::Result<T, Error>;

pub fn last_err() -> Error { Error::Win32(unsafe { GetLastError() }) }
```

- No `unwrap()` or `expect()` outside tests; no `format!` in common paths (both pull in formatting code).
- Top-level failures show one `MessageBoxW` and exit with a non-zero code.
- A missing `tasks.txt` is not an error: fall back to the embedded list.

## Windows API handling

- Each `unsafe` block lives in a wrapper of a few lines with a `// SAFETY:` comment stating the invariant.
- Strings: one helper, `to_wide(&str) -> Vec<u16>`, that appends the null terminator. Keep the Vec alive for the duration of the call.
- Registry: `RegOpenKeyExW` (HKEY_CURRENT_USER, `KEY_SET_VALUE | KEY_QUERY_VALUE`), `RegSetValueExW` with `REG_SZ` and the quoted path from `std::env::current_exe()`, `RegDeleteValueW`, `RegQueryValueExW`. Always `RegCloseKey`.
- Tray: `NOTIFYICONDATAW` with `NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_INFO`, `uCallbackMessage = WM_TRAY`. Handle `NIN_BALLOONUSERCLICK`, `WM_LBUTTONUP` and `WM_RBUTTONUP` from `lParam`.
- Context menu: `CreatePopupMenu` + `AppendMenuW`, then `SetForegroundWindow(hwnd)` before `TrackPopupMenu` so the menu closes on outside click.
- Always call `Shell_NotifyIconW(NIM_DELETE, ...)` in `WM_DESTROY` so no ghost icon remains.
- Re-add the tray icon on the `TaskbarCreated` message (`RegisterWindowMessageW`) so it survives an Explorer restart.

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

- `#![windows_subsystem = "windows"]` at the top of `main.rs` so no console window appears.
- Enable only the `windows-sys` features actually used.
- Check size regularly with `cargo bloat --release --crates`.
