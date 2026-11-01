use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_APP;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Error {
    Win32(u32),
    Io(std::io::ErrorKind),
}
pub type Result<T> = core::result::Result<T, Error>;

#[allow(dead_code)]
pub fn last_err() -> Error {
    // SAFETY: GetLastError has no preconditions.
    Error::Win32(unsafe { GetLastError() })
}

pub fn to_wide(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

pub const WM_TRAY: u32 = WM_APP + 1;
pub const ID_SHOW: u16 = 1001;
pub const ID_EDIT: u16 = 1002;
pub const ID_AUTOSTART: u16 = 1003;
pub const ID_EXIT: u16 = 1004;
pub const ID_DISMISS: u16 = 1005;
