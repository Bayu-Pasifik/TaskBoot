use crate::win::{to_wide, Error, Result};
use core::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::*;

const SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "TaskBoot";

struct Key(HKEY);

impl Key {
    fn open() -> Result<Key> {
        let sub = to_wide(SUBKEY);
        let mut h: HKEY = null_mut();
        // SAFETY: `sub` is NUL-terminated and outlives the call; `h` is a valid out pointer.
        let r = unsafe {
            RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_SET_VALUE | KEY_QUERY_VALUE, &mut h)
        };
        if r == ERROR_SUCCESS {
            Ok(Key(h))
        } else {
            Err(Error::Win32(r))
        }
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: self.0 is an open key obtained from RegOpenKeyExW.
        unsafe { RegCloseKey(self.0) };
    }
}

pub fn is_enabled() -> bool {
    let Ok(k) = Key::open() else { return false };
    let name = to_wide(NAME);
    // SAFETY: `name` is NUL-terminated; null data/size pointers are allowed for existence checks.
    unsafe { RegQueryValueExW(k.0, name.as_ptr(), null(), null_mut(), null_mut(), null_mut()) == ERROR_SUCCESS }
}

pub fn enable() -> Result<()> {
    let exe = std::env::current_exe().map_err(|e| Error::Io(e.kind()))?;
    let mut quoted = String::from("\"");
    quoted.push_str(&exe.to_string_lossy());
    quoted.push('"');
    let data = to_wide(&quoted);
    let k = Key::open()?;
    let name = to_wide(NAME);
    // SAFETY: `name` and `data` are NUL-terminated and live across the call; byte length matches `data`.
    let r = unsafe {
        RegSetValueExW(k.0, name.as_ptr(), 0, REG_SZ, data.as_ptr() as *const u8, (data.len() * 2) as u32)
    };
    if r == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(Error::Win32(r))
    }
}

pub fn disable() -> Result<()> {
    let k = Key::open()?;
    let name = to_wide(NAME);
    // SAFETY: `name` is NUL-terminated and outlives the call.
    let r = unsafe { RegDeleteValueW(k.0, name.as_ptr()) };
    if r == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(Error::Win32(r))
    }
}

pub fn toggle() -> Result<()> {
    if is_enabled() {
        disable()
    } else {
        enable()
    }
}
