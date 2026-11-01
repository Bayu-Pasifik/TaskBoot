#![windows_subsystem = "windows"]

mod autostart;
mod tasks;
mod tray;
mod win;
mod window;

use core::ptr::{null, null_mut};
use win::*;
use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Shell::{ShellExecuteW, NIN_BALLOONUSERCLICK};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

static mut WM_TASKBAR_CREATED: u32 = 0;

fn fail(msg: &str) -> ! {
    let m = to_wide(msg);
    let t = to_wide("TaskBoot");
    // SAFETY: strings are NUL-terminated and outlive the call.
    unsafe { MessageBoxW(null_mut(), m.as_ptr(), t.as_ptr(), MB_OK | MB_ICONERROR) };
    std::process::exit(1)
}

fn open_in_notepad() {
    let Some(p) = tasks::tasks_path() else { return };
    if !p.exists() {
        // Seed the file with defaults so Notepad opens something editable.
        let _ = std::fs::write(&p, include_str!("../assets/default_tasks.txt"));
    }
    let op = to_wide("open");
    let exe = to_wide("notepad.exe");
    let arg = to_wide(&p.to_string_lossy());
    // SAFETY: all strings NUL-terminated and outlive the call.
    unsafe { ShellExecuteW(null_mut(), op.as_ptr(), exe.as_ptr(), arg.as_ptr(), null(), SW_SHOWNORMAL) };
}

fn announce(hwnd: HWND) {
    let t = tasks::load();
    let mut title = t.len().to_string();
    title.push_str(if t.len() == 1 { " task today" } else { " tasks today" });
    tray::balloon(hwnd, &title, t.first().map(String::as_str).unwrap_or("No tasks"));
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    match msg {
        WM_TRAY => match (lparam as u32) & 0xFFFF {
            NIN_BALLOONUSERCLICK | WM_LBUTTONUP => window::show(hwnd),
            WM_RBUTTONUP => tray::show_menu(hwnd, autostart::is_enabled()),
            _ => {}
        },
        WM_COMMAND => match (wparam & 0xFFFF) as u16 {
            ID_SHOW => window::show(hwnd),
            ID_EDIT => open_in_notepad(),
            ID_AUTOSTART => {
                let _ = autostart::toggle();
            }
            ID_EXIT => {
                DestroyWindow(hwnd);
            }
            _ => {}
        },
        WM_DESTROY => {
            tray::remove(hwnd);
            PostQuitMessage(0);
        }
        _ if msg != 0 && msg == WM_TASKBAR_CREATED => tray::add(hwnd),
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    0
}

fn main() {
    let name = to_wide("Local\\TaskBootSingleton");
    // SAFETY: name is NUL-terminated; mutex handle is intentionally kept for the process lifetime.
    unsafe {
        let m = CreateMutexW(null(), 0, name.as_ptr());
        if m.is_null() {
            fail("Cannot create mutex");
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }

        let class = to_wide("TaskBootHidden");
        let inst = GetModuleHandleW(null());
        let mut wc: WNDCLASSW = core::mem::zeroed();
        wc.lpfnWndProc = Some(wnd_proc);
        wc.hInstance = inst;
        wc.lpszClassName = class.as_ptr();
        if RegisterClassW(&wc) == 0 {
            fail("Cannot register window class");
        }
        window::register();
        let tb = to_wide("TaskbarCreated");
        WM_TASKBAR_CREATED = RegisterWindowMessageW(tb.as_ptr());

        let title = to_wide("TaskBoot");
        let hwnd = CreateWindowExW(0, class.as_ptr(), title.as_ptr(), 0, 0, 0, 0, 0, null_mut(), null_mut(), inst, null());
        if hwnd.is_null() {
            fail("Cannot create window");
        }

        if !autostart::is_enabled() {
            let _ = autostart::enable();
        }
        tray::add(hwnd);
        announce(hwnd);

        let mut msg: MSG = core::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
