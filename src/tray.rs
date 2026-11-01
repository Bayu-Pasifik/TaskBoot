use crate::win::*;
use core::mem::{size_of, zeroed};
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const TIP: &str = "TaskBoot";

fn base(hwnd: HWND) -> NOTIFYICONDATAW {
    // SAFETY: NOTIFYICONDATAW is plain data; all-zero is a valid initial value.
    let mut nid: NOTIFYICONDATAW = unsafe { zeroed() };
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid
}

fn copy_wide(dst: &mut [u16], s: &str) {
    let max = dst.len() - 1;
    let mut n = 0;
    for u in s.encode_utf16().take(max) {
        dst[n] = u;
        n += 1;
    }
    dst[n] = 0;
}

pub fn add(hwnd: HWND) {
    let mut nid = base(hwnd);
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY;
    // SAFETY: resource id 1 is the embedded icon; null module name = this exe.
    nid.hIcon = unsafe {
        let inst = GetModuleHandleW(core::ptr::null());
        let ico = LoadImageW(inst, 1 as *const u16, IMAGE_ICON, 0, 0, LR_DEFAULTSIZE | LR_SHARED);
        if ico.is_null() {
            LoadIconW(core::ptr::null_mut(), IDI_APPLICATION)
        } else {
            ico
        }
    };
    copy_wide(&mut nid.szTip, TIP);
    // SAFETY: nid is fully initialised with correct cbSize.
    unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
}

pub fn balloon(hwnd: HWND, title: &str, body: &str) {
    let mut nid = base(hwnd);
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = NIIF_INFO;
    copy_wide(&mut nid.szInfoTitle, title);
    copy_wide(&mut nid.szInfo, body); // truncates to 255 UTF-16 units
    // SAFETY: nid is fully initialised with correct cbSize.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
}

pub fn remove(hwnd: HWND) {
    let nid = base(hwnd);
    // SAFETY: nid identifies the icon added by `add`.
    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
}

pub fn show_menu(hwnd: HWND, autostart_on: bool) {
    let show = to_wide("Show Tasks");
    let edit = to_wide("Edit Tasks");
    let auto = to_wide("Auto-start");
    let exit = to_wide("Exit");
    // SAFETY: strings outlive the calls; menu is destroyed before return.
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        AppendMenuW(menu, MF_STRING, ID_SHOW as usize, show.as_ptr());
        AppendMenuW(menu, MF_STRING, ID_EDIT as usize, edit.as_ptr());
        let flags = if autostart_on { MF_STRING | MF_CHECKED } else { MF_STRING };
        AppendMenuW(menu, flags, ID_AUTOSTART as usize, auto.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, core::ptr::null());
        AppendMenuW(menu, MF_STRING, ID_EXIT as usize, exit.as_ptr());
        let mut pt: POINT = zeroed();
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, core::ptr::null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }
}
