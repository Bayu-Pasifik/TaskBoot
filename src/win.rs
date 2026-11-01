#![allow(dead_code)]

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_APP;

#[derive(Debug, Clone, Copy)]
pub enum Error {
    Win32(u32),
    Io(std::io::ErrorKind),
}
pub type Result<T> = core::result::Result<T, Error>;

pub fn last_err() -> Error {
    // SAFETY: GetLastError has no preconditions.
    Error::Win32(unsafe { GetLastError() })
}

/// UTF-16 with trailing NUL. Keep the Vec alive for the duration of the call.
pub fn to_wide(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

pub const WM_TRAY: u32 = WM_APP + 1;
pub const TIMER_MAIN: usize = 1;

pub const ID_OPEN: u16 = 1001;
pub const ID_ADD: u16 = 1002;
pub const ID_MUTE_1H: u16 = 1003;
pub const ID_AUTOSTART: u16 = 1004;
pub const ID_EXIT: u16 = 1005;
pub const ID_DONE: u16 = 1006;
pub const ID_SNOOZE_10M: u16 = 1007;
pub const ID_SNOOZE_1H: u16 = 1008;
pub const ID_SNOOZE_TOMORROW: u16 = 1009;
pub const ID_EDIT: u16 = 1010;
pub const ID_DELETE: u16 = 1011;
