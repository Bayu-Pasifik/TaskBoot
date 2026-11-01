//! Task window (ListView, editor, context menu). Filled in at step 6.

use windows_sys::Win32::Foundation::HWND;

pub fn register() {}

/// Open the list window, optionally selecting a task.
pub fn show(_owner: HWND, _select: Option<u32>) {}

/// Open the list window with the add-task editor.
pub fn add(_owner: HWND) {}

/// Refresh the list if it is open.
pub fn refresh() {}
