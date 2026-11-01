use crate::win::*;
use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetStockObject, COLOR_WINDOW, DEFAULT_GUI_FONT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const CLASS: &str = "TaskBootList";
const W: i32 = 360;
const H: i32 = 420;

pub fn register() {
    // Leaked on purpose: class name must outlive the registered class.
    let class: &'static [u16] = Box::leak(to_wide(CLASS).into_boxed_slice());
    // SAFETY: WNDCLASSW is plain data; class name is 'static and NUL-terminated.
    unsafe {
        let mut wc: WNDCLASSW = core::mem::zeroed();
        wc.lpfnWndProc = Some(list_proc);
        wc.hInstance = GetModuleHandleW(core::ptr::null());
        wc.hbrBackground = (COLOR_WINDOW + 1) as _;
        wc.hCursor = LoadCursorW(core::ptr::null_mut(), IDC_ARROW);
        wc.lpszClassName = class.as_ptr();
        RegisterClassW(&wc);
    }
}

#[allow(clippy::too_many_arguments)]
fn child(parent: HWND, class: &str, text: &str, style: u32, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    let c = to_wide(class);
    let t = to_wide(text);
    // SAFETY: strings outlive the call; parent is a live window.
    unsafe {
        let hw = CreateWindowExW(
            0,
            c.as_ptr(),
            t.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            w,
            h,
            parent,
            id as usize as _,
            GetModuleHandleW(core::ptr::null()),
            core::ptr::null(),
        );
        SendMessageW(hw, WM_SETFONT, GetStockObject(DEFAULT_GUI_FONT) as WPARAM, 1);
        hw
    }
}

/// Show the task window, or bring the existing one to front.
pub fn show(_owner: HWND) {
    let class = to_wide(CLASS);
    // SAFETY: class is NUL-terminated; all handles come from the calls below.
    unsafe {
        let existing = FindWindowW(class.as_ptr(), core::ptr::null());
        if !existing.is_null() {
            ShowWindow(existing, SW_SHOW);
            SetForegroundWindow(existing);
            return;
        }
        let title = to_wide("TaskBoot");
        let x = (GetSystemMetrics(SM_CXSCREEN) - W) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - H) / 2;
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            x,
            y,
            W,
            H,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            GetModuleHandleW(core::ptr::null()),
            core::ptr::null(),
        );
        if hwnd.is_null() {
            return;
        }
        let mut rc: RECT = core::mem::zeroed();
        GetClientRect(hwnd, &mut rc);
        let (cw, ch) = (rc.right, rc.bottom);
        let list = child(hwnd, "LISTBOX", "", WS_BORDER | WS_VSCROLL | LBS_NOSEL as u32, 10, 10, cw - 20, ch - 60, 0);
        for t in crate::tasks::load() {
            let w = to_wide(&t);
            SendMessageW(list, LB_ADDSTRING, 0, w.as_ptr() as LPARAM);
        }
        child(hwnd, "BUTTON", "Dismiss", BS_DEFPUSHBUTTON as u32 | WS_TABSTOP, cw - 100, ch - 40, 90, 30, ID_DISMISS);
        SetForegroundWindow(hwnd);
    }
}

unsafe extern "system" fn list_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    match msg {
        WM_COMMAND if (wparam & 0xFFFF) as u16 == ID_DISMISS => {
            DestroyWindow(hwnd);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
