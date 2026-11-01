#![windows_subsystem = "windows"]

mod alert;
mod autostart;
mod clock;
mod model;
mod scheduler;
mod store;
mod tray;
mod win;
mod window;

use core::ptr::{null, null_mut};
use model::{Line, Minute, Settings, Status, Task};
use scheduler::AlertState;
use std::sync::atomic::{AtomicU32, Ordering};
use win::*;
use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Shell::NIN_BALLOONUSERCLICK;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub struct App {
    pub settings: Settings,
    pub lines: Vec<Line>,
    pub alert_state: AlertState,
    /// Task to select when the user clicks the last balloon.
    pub pending_select: Option<u32>,
    pub hwnd: HWND,
}

impl App {
    pub fn tasks(&self) -> Vec<Task> {
        model::tasks_of(&self.lines)
    }

    pub fn reload(&mut self) {
        if let Ok((settings, lines)) = store::load() {
            self.settings = settings;
            self.lines = lines;
        }
    }

    /// Save; on failure show one message box and keep the in-memory list.
    pub fn save(&self) {
        if store::save(&self.lines).is_err() {
            message_box("Could not save tasks.txt. Your changes are kept in memory.");
        }
    }

    pub fn any_overdue(&self, now: Minute) -> bool {
        self.lines.iter().any(|l| {
            matches!(l, Line::Task(t) if t.status == Status::Open
                && matches!(t.deadline, Some(d) if d <= now))
        })
    }

    pub fn next_id(&self) -> u32 {
        self.tasks().iter().map(|t| t.id).max().unwrap_or(0) + 1
    }
}

// The one piece of global state: the single `App`. Only touched from the UI thread.
static mut APP: Option<App> = None;
static WM_TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

pub fn app() -> Option<&'static mut App> {
    // SAFETY: single-threaded access from the message loop; callers keep borrows short.
    unsafe { (*core::ptr::addr_of_mut!(APP)).as_mut() }
}

pub fn message_box(msg: &str) {
    let m = to_wide(msg);
    let t = to_wide("TaskBoot");
    // SAFETY: strings are NUL-terminated and outlive the call.
    unsafe { MessageBoxW(null_mut(), m.as_ptr(), t.as_ptr(), MB_OK | MB_ICONERROR) };
}

fn fail(msg: &str) -> ! {
    message_box(msg);
    std::process::exit(1)
}

/// Run the scheduler, fire due alerts, update the tray icon and re-arm the timer.
pub fn tick(hwnd: HWND) {
    let Some(app) = app() else { return };
    // SAFETY: hwnd is our live hidden window.
    unsafe { KillTimer(hwnd, TIMER_MAIN) };
    if store::changed_on_disk() {
        app.reload();
        window::refresh();
    }
    let now = clock::now_local();
    let tasks = app.tasks();
    let events = scheduler::due_events(&tasks, &app.settings, &app.alert_state, now);
    for ev in &events {
        if let Some(id) = alert::fire(hwnd, ev, &tasks, now) {
            app.pending_select = Some(id);
        }
        scheduler::record(&mut app.alert_state, ev, &tasks, now);
    }
    app.alert_state.started = true;
    tray::set_overdue(hwnd, app.any_overdue(now));

    let wake = scheduler::next_wake(&tasks, &app.settings, &app.alert_state, now);
    let mins = wake.0.saturating_sub(now.0).clamp(1, 60);
    // Align to the minute boundary: `now` has dropped the seconds already elapsed.
    let ms = (mins * 60_000)
        .saturating_sub(clock::seconds_into_minute() as u64 * 1000)
        .max(1000) as u32;
    // SAFETY: hwnd is our live hidden window; no callback (WM_TIMER is posted).
    unsafe { SetTimer(hwnd, TIMER_MAIN, ms, None) };
}

fn mute_toggle(hwnd: HWND) {
    if let Some(app) = app() {
        let now = clock::now_local();
        app.alert_state.mute_until =
            if app.alert_state.is_muted(now) { None } else { Some(Minute(now.0 + 60)) };
    }
    tick(hwnd);
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    match msg {
        WM_TRAY => match (lparam as u32) & 0xFFFF {
            NIN_BALLOONUSERCLICK => {
                let sel = app().and_then(|a| a.pending_select.take());
                window::show(hwnd, sel);
            }
            WM_LBUTTONUP => window::show(hwnd, None),
            WM_RBUTTONUP => {
                let muted = app().map(|a| a.alert_state.is_muted(clock::now_local())).unwrap_or(false);
                tray::show_menu(hwnd, autostart::is_enabled(), muted);
            }
            _ => {}
        },
        WM_COMMAND => match (wparam & 0xFFFF) as u16 {
            ID_OPEN => window::show(hwnd, None),
            ID_ADD => window::add(hwnd),
            ID_MUTE_1H => mute_toggle(hwnd),
            ID_AUTOSTART => {
                let _ = autostart::toggle();
            }
            ID_EXIT => {
                DestroyWindow(hwnd);
            }
            _ => {}
        },
        WM_TIMER if wparam == TIMER_MAIN => tick(hwnd),
        WM_TIMECHANGE => tick(hwnd),
        WM_POWERBROADCAST => {
            if wparam as u32 == PBT_APMRESUMEAUTOMATIC {
                // Treat resume like a login: one summary of everything missed.
                if let Some(a) = app() {
                    a.alert_state.started = false;
                }
                tick(hwnd);
            }
            return 1;
        }
        WM_DESTROY => {
            tray::remove(hwnd);
            KillTimer(hwnd, TIMER_MAIN);
            PostQuitMessage(0);
        }
        _ if msg != 0 && msg == WM_TASKBAR_CREATED.load(Ordering::Relaxed) => {
            let overdue = app().map(|a| a.any_overdue(clock::now_local())).unwrap_or(false);
            tray::add(hwnd, overdue);
        }
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    0
}

fn main() {
    let name = to_wide("Local\\TaskBootSingleton");
    // SAFETY: name is NUL-terminated; the mutex handle is kept for the process lifetime.
    unsafe {
        let m = CreateMutexW(null(), 0, name.as_ptr());
        if m.is_null() {
            fail("Cannot create mutex");
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }

        let (settings, lines) = match store::load() {
            Ok(x) => x,
            Err(_) => {
                message_box("Cannot read or create tasks.txt next to the exe.");
                (Settings::default(), Vec::new())
            }
        };
        *core::ptr::addr_of_mut!(APP) = Some(App {
            settings,
            lines,
            alert_state: AlertState::default(),
            pending_select: None,
            hwnd: null_mut(),
        });

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
        WM_TASKBAR_CREATED.store(RegisterWindowMessageW(tb.as_ptr()), Ordering::Relaxed);

        let title = to_wide("TaskBoot");
        let hwnd = CreateWindowExW(0, class.as_ptr(), title.as_ptr(), 0, 0, 0, 0, 0, null_mut(), null_mut(), inst, null());
        if hwnd.is_null() {
            fail("Cannot create window");
        }
        if let Some(a) = app() {
            a.hwnd = hwnd;
        }

        if !autostart::is_enabled() {
            let _ = autostart::enable();
        }
        let overdue = app().map(|a| a.any_overdue(clock::now_local())).unwrap_or(false);
        tray::add(hwnd, overdue);
        tick(hwnd);

        let mut msg: MSG = core::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
