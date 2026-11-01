use crate::win::*;
use core::mem::{size_of, zeroed};
use core::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const TIP: &str = "TaskBoot";
const ICON_NORMAL: usize = 1;
const ICON_ALERT: usize = 2;

fn base(hwnd: HWND) -> NOTIFYICONDATAW {
    // SAFETY: NOTIFYICONDATAW is plain data; all-zero is a valid initial value.
    let mut nid: NOTIFYICONDATAW = unsafe { zeroed() };
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid
}

/// Copy into a fixed buffer, truncating to fit and always NUL-terminating.
fn copy_wide(dst: &mut [u16], s: &str) {
    let max = dst.len() - 1;
    let mut n = 0;
    for u in s.encode_utf16().take(max) {
        dst[n] = u;
        n += 1;
    }
    dst[n] = 0;
}

fn load_icon(id: usize) -> HICON {
    // SAFETY: id is an embedded icon resource id (MAKEINTRESOURCE); null module name = this exe.
    unsafe {
        let inst = GetModuleHandleW(null());
        let ico = LoadImageW(
            inst,
            id as *const u16,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_SHARED,
        );
        if ico.is_null() {
            LoadIconW(null_mut(), IDI_APPLICATION)
        } else {
            ico
        }
    }
}

pub fn add(hwnd: HWND, overdue: bool) {
    let mut nid = base(hwnd);
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = load_icon(if overdue { ICON_ALERT } else { ICON_NORMAL });
    copy_wide(&mut nid.szTip, TIP);
    // SAFETY: nid is fully initialised with the right cbSize.
    unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
}

pub fn set_overdue(hwnd: HWND, overdue: bool) {
    let mut nid = base(hwnd);
    nid.uFlags = NIF_ICON | NIF_TIP;
    nid.hIcon = load_icon(if overdue { ICON_ALERT } else { ICON_NORMAL });
    copy_wide(&mut nid.szTip, if overdue { "TaskBoot - overdue tasks" } else { TIP });
    // SAFETY: nid is fully initialised with the right cbSize.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
}

/// Balloon notification (shown as a toast on Windows 10/11). Text is truncated to the shell limits.
pub fn balloon(hwnd: HWND, title: &str, body: &str, warning: bool) {
    let mut nid = base(hwnd);
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = if warning { NIIF_WARNING } else { NIIF_INFO };
    copy_wide(&mut nid.szInfoTitle, title);
    copy_wide(&mut nid.szInfo, body);
    // SAFETY: nid is fully initialised with the right cbSize.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
}

pub fn remove(hwnd: HWND) {
    let nid = base(hwnd);
    // SAFETY: nid identifies the icon added by `add`.
    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
}

pub fn show_menu(hwnd: HWND, autostart_on: bool, muted: bool) {
    let items = [
        (ID_OPEN, "Open list", false),
        (ID_ADD, "Add task", false),
        (ID_MUTE_1H, "Mute alerts for 1 h", muted),
        (ID_AUTOSTART, "Auto-start", autostart_on),
    ];
    let exit = to_wide("Exit");
    // SAFETY: wide strings outlive each call; the menu is destroyed before returning.
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        for (id, text, checked) in items {
            let w = to_wide(text);
            let flags = if checked { MF_STRING | MF_CHECKED } else { MF_STRING };
            AppendMenuW(menu, flags, id as usize, w.as_ptr());
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        AppendMenuW(menu, MF_STRING, ID_EXIT as usize, exit.as_ptr());
        let mut pt: POINT = zeroed();
        GetCursorPos(&mut pt);
        // Required so the menu closes when the user clicks elsewhere.
        SetForegroundWindow(hwnd);
        TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }
}
