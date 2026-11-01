# TaskBoot

Single portable `.exe` (~230 KB) that shows your job tasks at every Windows login, then idles at 0% CPU.

## Use

1. Put `taskboot.exe` and `tasks.txt` in a folder you will keep.
2. Run `taskboot.exe` once. It registers itself under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
3. At each login: tray icon + balloon ("N tasks today"). Click the balloon or tray icon to open the task list.

`tasks.txt`: one task per line, `#` lines ignored. Missing file uses a built-in default list. Edits apply on next launch.

Tray menu: Show Tasks, Edit Tasks, Auto-start on/off, Exit.

## Remove

Turn **Auto-start off** in the tray menu *before* deleting the exe, or the Run entry stays behind.

## Build

```
cargo test
cargo build --release
```

Windows 10/11, x86_64, MSVC toolchain.
