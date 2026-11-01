//! Task list window: ListView, add/edit dialog, context menu.
//! All state lives in `App`; window handles are found with FindWindow/GetDlgItem.

use crate::clock;
use crate::model::{Line, Minute, Status, Task};
use crate::win::*;
use crate::{app, tick};
use core::mem::{size_of, zeroed};
use core::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, SYSTEMTIME, WPARAM};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const LIST_CLASS: &str = "TaskBootList";
const EDIT_CLASS: &str = "TaskBootEdit";
const LIST_ID: i32 = 2001;
const TITLE_ID: i32 = 2101;
const PICKER_ID: i32 = 2102;
const IDOK: i32 = 1;
const IDCANCEL: i32 = 2;
const RED: u32 = 0x0000_00C8; // COLORREF 0x00BBGGRR

// ---------------------------------------------------------------- pure helpers

/// Display order: overdue, then nearest deadline, then no deadline, then done.
pub fn sort_key(t: &Task, now: Minute) -> (u8, u64, u32) {
    if t.status == Status::Done {
        return (3, t.deadline.map(|d| d.0).unwrap_or(u64::MAX), t.id);
    }
    match t.deadline {
        Some(d) if d <= now => (0, d.0, t.id),
        Some(d) => (1, d.0, t.id),
        None => (2, 0, t.id),
    }
}

pub fn is_overdue(t: &Task, now: Minute) -> bool {
    t.status == Status::Open && matches!(t.deadline, Some(d) if d <= now)
}

fn deadline_text(t: &Task, now: Minute) -> String {
    let mut s = match t.deadline {
        Some(d) => clock::format(d),
        None => String::new(),
    };
    if is_overdue(t, now) {
        s.push_str("  OVERDUE");
    }
    s
}

fn clean_title(s: &str) -> String {
    s.trim().replace('|', "/").replace(['\r', '\n'], " ")
}

// ---------------------------------------------------------------- Win32 wrappers

fn dpi_of(hwnd: HWND) -> u32 {
    // SAFETY: both calls only read DPI state; a null hwnd is rejected by returning 0.
    let d = unsafe {
        if hwnd.is_null() {
            GetDpiForSystem()
        } else {
            GetDpiForWindow(hwnd)
        }
    };
    if d == 0 {
        96
    } else {
        d
    }
}

fn scale(dpi: u32, px: i32) -> i32 {
    px * dpi as i32 / 96
}

fn make_font(dpi: u32) -> HFONT {
    // SAFETY: NONCLIENTMETRICSW is plain data; cbSize is set before the call.
    unsafe {
        let mut ncm: NONCLIENTMETRICSW = zeroed();
        ncm.cbSize = size_of::<NONCLIENTMETRICSW>() as u32;
        if SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS, ncm.cbSize, &mut ncm as *mut _ as *mut _, 0, dpi) != 0 {
            CreateFontIndirectW(&ncm.lfMessageFont)
        } else {
            GetStockObject(DEFAULT_GUI_FONT) as HFONT
        }
    }
}

fn prop_set(hwnd: HWND, name: &str, v: isize) {
    let n = to_wide(name);
    // SAFETY: n is NUL-terminated and outlives the call.
    unsafe { SetPropW(hwnd, n.as_ptr(), v as _) };
}

fn prop_get(hwnd: HWND, name: &str) -> isize {
    let n = to_wide(name);
    // SAFETY: n is NUL-terminated and outlives the call.
    unsafe { GetPropW(hwnd, n.as_ptr()) as isize }
}

fn prop_remove(hwnd: HWND, name: &str) -> isize {
    let n = to_wide(name);
    // SAFETY: n is NUL-terminated and outlives the call.
    unsafe { RemovePropW(hwnd, n.as_ptr()) as isize }
}

#[allow(clippy::too_many_arguments)]
fn child(parent: HWND, font: HFONT, class: &str, text: &str, style: u32, x: i32, y: i32, w: i32, h: i32, id: i32) -> HWND {
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
            GetModuleHandleW(null()),
            null(),
        );
        SendMessageW(hw, WM_SETFONT, font as WPARAM, 1);
        hw
    }
}

fn find_list() -> HWND {
    let c = to_wide(LIST_CLASS);
    // SAFETY: c is NUL-terminated and outlives the call.
    unsafe { FindWindowW(c.as_ptr(), null()) }
}

fn listview(hwnd: HWND) -> HWND {
    // SAFETY: plain handle lookup.
    unsafe { GetDlgItem(hwnd, LIST_ID) }
}

fn message_box(owner: HWND, text: &str, flags: u32) -> i32 {
    let t = to_wide(text);
    let c = to_wide("TaskBoot");
    // SAFETY: strings are NUL-terminated and outlive the call.
    unsafe { MessageBoxW(owner, t.as_ptr(), c.as_ptr(), flags) }
}

// ---------------------------------------------------------------- registration

pub fn register() {
    // SAFETY: plain init calls; class names are leaked so they outlive the classes.
    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_DATE_CLASSES,
        };
        InitCommonControlsEx(&icc);
        for (name, proc_) in [
            (LIST_CLASS, list_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT),
            (EDIT_CLASS, edit_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT),
        ] {
            let class: &'static [u16] = Box::leak(to_wide(name).into_boxed_slice());
            let mut wc: WNDCLASSW = zeroed();
            wc.lpfnWndProc = Some(proc_);
            wc.hInstance = GetModuleHandleW(null());
            wc.hbrBackground = (COLOR_BTNFACE + 1) as _;
            wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
            wc.lpszClassName = class.as_ptr();
            RegisterClassW(&wc);
        }
    }
}

// ---------------------------------------------------------------- list window

/// Open the list window (or bring it to the front) and optionally select a task.
pub fn show(_owner: HWND, select: Option<u32>) {
    let mut hwnd = find_list();
    if hwnd.is_null() {
        hwnd = create_list();
        if hwnd.is_null() {
            return;
        }
    }
    refresh();
    if let Some(id) = select {
        select_id(listview(hwnd), id);
    }
    // SAFETY: hwnd is a live window.
    unsafe {
        ShowWindow(hwnd, SW_SHOWNORMAL);
        SetForegroundWindow(hwnd);
    }
}

/// Open the list window with the add-task editor.
pub fn add(owner: HWND) {
    show(owner, None);
    let list = find_list();
    if !list.is_null() {
        edit_dialog(list, None);
    }
}

fn create_list() -> HWND {
    let class = to_wide(LIST_CLASS);
    let title = to_wide("TaskBoot - Tasks");
    let dpi = dpi_of(null_mut());
    let (w, h) = (scale(dpi, 560), scale(dpi, 470));
    // SAFETY: strings outlive the calls; all handles come from the calls below.
    unsafe {
        let x = (GetSystemMetrics(SM_CXSCREEN) - w) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - h) / 2;
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            x,
            y,
            w,
            h,
            null_mut(),
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        );
        if hwnd.is_null() {
            return hwnd;
        }
        let dpi = dpi_of(hwnd);
        let font = make_font(dpi);
        prop_set(hwnd, "tbfont", font as isize);
        let lv = child(
            hwnd,
            font,
            "SysListView32",
            "",
            WS_BORDER | WS_TABSTOP | (LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS) as u32,
            0,
            0,
            10,
            10,
            LIST_ID,
        );
        SendMessageW(
            lv,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_CHECKBOXES | LVS_EX_FULLROWSELECT) as LPARAM,
        );
        for (i, (name, wpx)) in [("Task", 330), ("Deadline", 170)].into_iter().enumerate() {
            let text = to_wide(name);
            let mut col: LVCOLUMNW = zeroed();
            col.mask = LVCF_TEXT | LVCF_WIDTH;
            col.cx = scale(dpi, wpx);
            col.pszText = text.as_ptr() as *mut u16;
            SendMessageW(lv, LVM_INSERTCOLUMNW, i, &col as *const _ as LPARAM);
        }
        for (id, text) in [(ID_ADD, "Add"), (ID_EDIT, "Edit"), (ID_DELETE, "Delete")] {
            child(hwnd, font, "BUTTON", text, WS_TABSTOP | BS_PUSHBUTTON as u32, 0, 0, 10, 10, id as i32);
        }
        layout(hwnd);
        hwnd
    }
}

fn layout(hwnd: HWND) {
    // SAFETY: hwnd is a live window; child handles come from GetDlgItem.
    unsafe {
        let mut rc: RECT = zeroed();
        GetClientRect(hwnd, &mut rc);
        let dpi = dpi_of(hwnd);
        let m = scale(dpi, 10);
        let (bw, bh) = (scale(dpi, 84), scale(dpi, 30));
        let lv_h = (rc.bottom - bh - m * 3).max(10);
        MoveWindow(listview(hwnd), m, m, (rc.right - m * 2).max(10), lv_h, 1);
        for (i, id) in [ID_ADD, ID_EDIT, ID_DELETE].into_iter().enumerate() {
            MoveWindow(GetDlgItem(hwnd, id as i32), m + i as i32 * (bw + m), rc.bottom - bh - m, bw, bh, 1);
        }
    }
}

fn insert_row(lv: HWND, row: usize, t: &Task, now: Minute) {
    let title = to_wide(&t.title);
    let dl = to_wide(&deadline_text(t, now));
    // SAFETY: LVITEMW is plain data; text buffers outlive the calls.
    unsafe {
        let mut it: LVITEMW = zeroed();
        it.mask = LVIF_TEXT | LVIF_PARAM;
        it.iItem = row as i32;
        it.pszText = title.as_ptr() as *mut u16;
        it.lParam = t.id as isize;
        SendMessageW(lv, LVM_INSERTITEMW, 0, &it as *const _ as LPARAM);
        let mut sub: LVITEMW = zeroed();
        sub.iSubItem = 1;
        sub.pszText = dl.as_ptr() as *mut u16;
        SendMessageW(lv, LVM_SETITEMTEXTW, row, &sub as *const _ as LPARAM);
        if t.status == Status::Done {
            set_checked(lv, row, true);
        }
    }
}

fn set_checked(lv: HWND, row: usize, checked: bool) {
    // SAFETY: LVITEMW is plain data; state image index 2 = checked, 1 = unchecked.
    unsafe {
        let mut it: LVITEMW = zeroed();
        it.stateMask = LVIS_STATEIMAGEMASK;
        it.state = (if checked { 2u32 } else { 1u32 }) << 12;
        SendMessageW(lv, LVM_SETITEMSTATE, row, &it as *const _ as LPARAM);
    }
}

fn row_id(lv: HWND, row: i32) -> Option<u32> {
    if row < 0 {
        return None;
    }
    // SAFETY: LVITEMW is plain data; the control fills lParam.
    unsafe {
        let mut it: LVITEMW = zeroed();
        it.mask = LVIF_PARAM;
        it.iItem = row;
        if SendMessageW(lv, LVM_GETITEMW, 0, &mut it as *mut _ as LPARAM) == 0 {
            return None;
        }
        Some(it.lParam as u32)
    }
}

fn selected_id(lv: HWND) -> Option<u32> {
    // SAFETY: plain message send.
    let row = unsafe { SendMessageW(lv, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED as LPARAM) } as i32;
    row_id(lv, row)
}

fn select_id(lv: HWND, id: u32) {
    // SAFETY: LVFINDINFOW is plain data.
    unsafe {
        let mut fi: LVFINDINFOW = zeroed();
        fi.flags = LVFI_PARAM;
        fi.lParam = id as isize;
        let row = SendMessageW(lv, LVM_FINDITEMW, usize::MAX, &fi as *const _ as LPARAM) as i32;
        if row >= 0 {
            let mut it: LVITEMW = zeroed();
            it.stateMask = LVIS_SELECTED | LVIS_FOCUSED;
            it.state = LVIS_SELECTED | LVIS_FOCUSED;
            SendMessageW(lv, LVM_SETITEMSTATE, row as usize, &it as *const _ as LPARAM);
            SendMessageW(lv, LVM_ENSUREVISIBLE, row as usize, 0);
        }
    }
}

/// Rebuild the list from the App state if the window is open.
pub fn refresh() {
    let hwnd = find_list();
    if hwnd.is_null() {
        return;
    }
    let Some(app) = app() else { return };
    let lv = listview(hwnd);
    let keep = selected_id(lv);
    let now = clock::now_local();
    let mut tasks = app.tasks();
    tasks.sort_by_key(|t| sort_key(t, now));
    prop_set(hwnd, "tbbusy", 1);
    // SAFETY: plain message sends on a live control.
    unsafe {
        SendMessageW(lv, WM_SETREDRAW, 0, 0);
        SendMessageW(lv, LVM_DELETEALLITEMS, 0, 0);
        for (i, t) in tasks.iter().enumerate() {
            insert_row(lv, i, t, now);
        }
        SendMessageW(lv, WM_SETREDRAW, 1, 0);
    }
    prop_set(hwnd, "tbbusy", 0);
    if let Some(id) = keep {
        select_id(lv, id);
    }
}

// ---------------------------------------------------------------- actions

fn after_change(id: Option<u32>) {
    let Some(app) = app() else { return };
    if let Some(id) = id {
        app.alert_state.forget(id);
    }
    app.save();
    refresh();
    let hwnd = app.hwnd;
    tick(hwnd);
}

fn with_task(id: u32, f: impl FnOnce(&mut Task)) {
    let Some(app) = app() else { return };
    for l in app.lines.iter_mut() {
        if let Line::Task(t) = l {
            if t.id == id {
                f(t);
                return;
            }
        }
    }
}

fn set_done(id: u32, done: bool) {
    with_task(id, |t| {
        t.status = if done { Status::Done } else { Status::Open };
        t.snooze = None;
    });
    after_change(Some(id));
}

fn snooze(id: u32, until: Minute) {
    with_task(id, |t| t.snooze = Some(until));
    after_change(Some(id));
}

fn delete(owner: HWND, id: u32) {
    if message_box(owner, "Delete this task?", MB_YESNO | MB_ICONQUESTION) != IDYES {
        return;
    }
    if let Some(app) = app() {
        app.lines.retain(|l| !matches!(l, Line::Task(t) if t.id == id));
    }
    after_change(Some(id));
}

fn task_by_id(id: u32) -> Option<Task> {
    app().and_then(|a| a.tasks().into_iter().find(|t| t.id == id))
}

fn do_edit(list: HWND, id: u32) {
    let Some(t) = task_by_id(id) else { return };
    if let Some((title, deadline)) = edit_dialog(list, Some(&t)) {
        with_task(id, |t| {
            t.title = title;
            t.deadline = deadline;
        });
        after_change(Some(id));
        select_id(listview(list), id);
    }
}

fn context_menu(list: HWND, id: u32) {
    let Some(t) = task_by_id(id) else { return };
    let items: [(u16, &str); 6] = [
        (ID_DONE, if t.status == Status::Done { "Mark open" } else { "Done" }),
        (ID_SNOOZE_10M, "Snooze 10 min"),
        (ID_SNOOZE_1H, "Snooze 1 h"),
        (ID_SNOOZE_TOMORROW, "Snooze until tomorrow 09:00"),
        (ID_EDIT, "Edit"),
        (ID_DELETE, "Delete"),
    ];
    // SAFETY: wide strings outlive each call; the menu is destroyed before returning.
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        for (i, (cid, text)) in items.iter().enumerate() {
            if i == 4 {
                AppendMenuW(menu, MF_SEPARATOR, 0, null());
            }
            let w = to_wide(text);
            AppendMenuW(menu, MF_STRING, *cid as usize, w.as_ptr());
        }
        let mut pt = zeroed();
        GetCursorPos(&mut pt);
        SetForegroundWindow(list);
        let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pt.x, pt.y, 0, list, null());
        DestroyMenu(menu);
        if cmd > 0 {
            SendMessageW(list, WM_COMMAND, cmd as WPARAM, 0);
        }
    }
}

fn handle_command(hwnd: HWND, cmd: u16) {
    let lv = listview(hwnd);
    if cmd == ID_ADD {
        edit_dialog(hwnd, None);
        return;
    }
    let Some(id) = selected_id(lv) else { return };
    let now = clock::now_local();
    match cmd {
        ID_EDIT => do_edit(hwnd, id),
        ID_DELETE => delete(hwnd, id),
        ID_DONE => {
            let done = task_by_id(id).map(|t| t.status == Status::Done).unwrap_or(false);
            set_done(id, !done);
        }
        ID_SNOOZE_10M => snooze(id, Minute(now.0 + 10)),
        ID_SNOOZE_1H => snooze(id, Minute(now.0 + 60)),
        ID_SNOOZE_TOMORROW => snooze(id, Minute(clock::day_start(now).0 + 1440 + 9 * 60)),
        _ => {}
    }
}

/// Add (None) or edit (Some) via the editor dialog; saves new tasks itself.
fn edit_dialog(owner: HWND, existing: Option<&Task>) -> Option<(String, Option<Minute>)> {
    let (title, deadline) = run_editor(owner, existing)?;
    if existing.is_none() {
        if let Some(app) = app() {
            let id = app.next_id();
            app.lines.push(Line::Task(Task { id, title, deadline, status: Status::Open, snooze: None }));
            after_change(Some(id));
            select_id(listview(owner), id);
        }
        return None;
    }
    Some((title, deadline))
}

// ---------------------------------------------------------------- list window proc

fn systemtime_to_minute(st: &SYSTEMTIME) -> Option<Minute> {
    clock::from_parts(st.wYear as u64, st.wMonth as u64, st.wDay as u64, st.wHour as u64, st.wMinute as u64)
}

unsafe extern "system" fn list_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            layout(hwnd);
            0
        }
        WM_COMMAND => {
            handle_command(hwnd, (wparam & 0xFFFF) as u16);
            0
        }
        WM_NOTIFY => {
            let hdr = &*(lparam as *const NMHDR);
            if hdr.idFrom != LIST_ID as usize {
                return 0;
            }
            match hdr.code {
                NM_RCLICK => {
                    let a = &*(lparam as *const NMITEMACTIVATE);
                    if let Some(id) = row_id(hdr.hwndFrom, a.iItem) {
                        select_id(hdr.hwndFrom, id);
                        context_menu(hwnd, id);
                    }
                }
                NM_DBLCLK => {
                    let a = &*(lparam as *const NMITEMACTIVATE);
                    if let Some(id) = row_id(hdr.hwndFrom, a.iItem) {
                        do_edit(hwnd, id);
                    }
                }
                LVN_ITEMCHANGED => {
                    let n = &*(lparam as *const NMLISTVIEW);
                    let old = (n.uOldState & LVIS_STATEIMAGEMASK) >> 12;
                    let new = (n.uNewState & LVIS_STATEIMAGEMASK) >> 12;
                    let busy = prop_get(hwnd, "tbbusy") != 0;
                    if !busy && n.uChanged & LVIF_STATE != 0 && old != 0 && new != 0 && old != new {
                        if let Some(id) = row_id(hdr.hwndFrom, n.iItem) {
                            set_done(id, new == 2);
                        }
                    }
                }
                NM_CUSTOMDRAW => {
                    let cd = &mut *(lparam as *mut NMLVCUSTOMDRAW);
                    return match cd.nmcd.dwDrawStage {
                        CDDS_PREPAINT => CDRF_NOTIFYITEMDRAW as LRESULT,
                        CDDS_ITEMPREPAINT => {
                            let id = cd.nmcd.lItemlParam as u32;
                            if task_by_id(id).map(|t| is_overdue(&t, clock::now_local())).unwrap_or(false) {
                                cd.clrText = RED;
                            }
                            CDRF_NEWFONT as LRESULT
                        }
                        _ => CDRF_DODEFAULT as LRESULT,
                    };
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            let f = prop_remove(hwnd, "tbfont");
            if f != 0 {
                DeleteObject(f as HGDIOBJ);
            }
            prop_remove(hwnd, "tbbusy");
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ---------------------------------------------------------------- editor dialog

struct EditState {
    done: bool,
    ok: bool,
    title: String,
    deadline: Option<Minute>,
}

fn run_editor(owner: HWND, existing: Option<&Task>) -> Option<(String, Option<Minute>)> {
    let class = to_wide(EDIT_CLASS);
    let caption = to_wide(if existing.is_some() { "Edit task" } else { "Add task" });
    let dpi = dpi_of(owner);
    let (w, h) = (scale(dpi, 400), scale(dpi, 210));
    let state = Box::into_raw(Box::new(EditState { done: false, ok: false, title: String::new(), deadline: None }));
    // SAFETY: `state` stays valid until freed at the end of this function; all other pointers
    // come from the calls below and the message loop is single-threaded.
    unsafe {
        let mut orc: RECT = zeroed();
        GetWindowRect(owner, &mut orc);
        let x = orc.left + ((orc.right - orc.left) - w) / 2;
        let y = orc.top + ((orc.bottom - orc.top) - h) / 2;
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class.as_ptr(),
            caption.as_ptr(),
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            x,
            y,
            w,
            h,
            owner,
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        );
        if hwnd.is_null() {
            drop(Box::from_raw(state));
            return None;
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        build_editor(hwnd, existing);
        EnableWindow(owner, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);

        let mut msg: MSG = zeroed();
        while !(*state).done {
            let r = GetMessageW(&mut msg, null_mut(), 0, 0);
            if r == 0 {
                PostQuitMessage(msg.wParam as i32);
                break;
            }
            if r < 0 {
                break;
            }
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        EnableWindow(owner, 1);
        SetForegroundWindow(owner);
        let st = Box::from_raw(state);
        if st.ok {
            Some((st.title, st.deadline))
        } else {
            None
        }
    }
}

fn build_editor(hwnd: HWND, existing: Option<&Task>) {
    let dpi = dpi_of(hwnd);
    let font = make_font(dpi);
    prop_set(hwnd, "tbfont", font as isize);
    let s = |px| scale(dpi, px);
    let title = existing.map(|t| t.title.as_str()).unwrap_or("");
    // SAFETY: SYSTEMTIME is plain data; handles come from `child`.
    unsafe {
        child(hwnd, font, "STATIC", "Title", 0, s(16), s(12), s(360), s(20), -1);
        child(hwnd, font, "EDIT", title, WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32, s(16), s(34), s(360), s(26), TITLE_ID);
        child(hwnd, font, "STATIC", "Deadline (untick the box for none)", 0, s(16), s(72), s(360), s(20), -1);
        let pk = child(
            hwnd,
            font,
            "SysDateTimePick32",
            "",
            WS_TABSTOP | DTS_SHOWNONE as u32 | DTS_UPDOWN as u32,
            s(16),
            s(94),
            s(220),
            s(26),
            PICKER_ID,
        );
        let fmt = to_wide("yyyy-MM-dd  HH:mm");
        SendMessageW(pk, DTM_SETFORMATW, 0, fmt.as_ptr() as LPARAM);
        match existing.and_then(|t| t.deadline) {
            Some(d) => {
                let days = d.0 / 1440;
                let rem = d.0 % 1440;
                let (y, mo, da) = clock::civil_from_days(days);
                let mut st: SYSTEMTIME = zeroed();
                st.wYear = y as u16;
                st.wMonth = mo as u16;
                st.wDay = da as u16;
                st.wHour = (rem / 60) as u16;
                st.wMinute = (rem % 60) as u16;
                SendMessageW(pk, DTM_SETSYSTEMTIME, GDT_VALID as WPARAM, &st as *const _ as LPARAM);
            }
            None => {
                SendMessageW(pk, DTM_SETSYSTEMTIME, GDT_NONE as WPARAM, 0);
            }
        }
        child(hwnd, font, "BUTTON", "OK", WS_TABSTOP | BS_DEFPUSHBUTTON as u32, s(196), s(136), s(86), s(30), IDOK);
        child(hwnd, font, "BUTTON", "Cancel", WS_TABSTOP | BS_PUSHBUTTON as u32, s(290), s(136), s(86), s(30), IDCANCEL);
        SetFocus(GetDlgItem(hwnd, TITLE_ID));
        SendMessageW(GetDlgItem(hwnd, TITLE_ID), EM_SETSEL, 0, -1);
    }
}

fn finish_edit(hwnd: HWND, ok: bool) {
    // SAFETY: GWLP_USERDATA holds the EditState pointer set in run_editor; it outlives this window.
    unsafe {
        let st = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut EditState;
        if st.is_null() {
            return;
        }
        if ok {
            let mut buf = [0u16; 256];
            let n = GetWindowTextW(GetDlgItem(hwnd, TITLE_ID), buf.as_mut_ptr(), buf.len() as i32);
            let title = clean_title(&String::from_utf16_lossy(&buf[..n.max(0) as usize]));
            if title.is_empty() {
                message_box(hwnd, "A title is required.", MB_OK | MB_ICONINFORMATION);
                SetFocus(GetDlgItem(hwnd, TITLE_ID));
                return;
            }
            let mut stime: SYSTEMTIME = zeroed();
            let r = SendMessageW(GetDlgItem(hwnd, PICKER_ID), DTM_GETSYSTEMTIME, 0, &mut stime as *mut _ as LPARAM);
            (*st).title = title;
            (*st).deadline = if r as u32 == GDT_VALID { systemtime_to_minute(&stime) } else { None };
        }
        (*st).ok = ok;
        (*st).done = true;
        // Re-enable the owner before this window goes away so activation returns to it.
        DestroyWindow(hwnd);
    }
}

unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            match (wparam & 0xFFFF) as i32 {
                IDOK => finish_edit(hwnd, true),
                IDCANCEL => finish_edit(hwnd, false),
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            finish_edit(hwnd, false);
            0
        }
        WM_DESTROY => {
            let f = prop_remove(hwnd, "tbfont");
            if f != 0 {
                DeleteObject(f as HGDIOBJ);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(s: &str) -> Minute {
        clock::parse(s).unwrap()
    }

    fn t(id: u32, d: Option<&str>, status: Status) -> Task {
        Task { id, title: "t".into(), deadline: d.map(m), status, snooze: None }
    }

    #[test]
    fn sort_order() {
        let now = m("2026-11-03 12:00");
        let mut v = vec![
            t(1, None, Status::Open),
            t(2, Some("2026-11-03 18:00"), Status::Open),
            t(3, Some("2026-11-03 09:00"), Status::Open),
            t(4, Some("2026-11-03 10:00"), Status::Done),
            t(5, Some("2026-11-03 13:00"), Status::Open),
            t(6, Some("2026-11-02 09:00"), Status::Open),
        ];
        v.sort_by_key(|x| sort_key(x, now));
        let ids: Vec<u32> = v.iter().map(|x| x.id).collect();
        assert_eq!(ids, [6, 3, 5, 2, 1, 4]);
    }

    #[test]
    fn overdue_text() {
        let now = m("2026-11-03 12:00");
        assert!(deadline_text(&t(1, Some("2026-11-03 11:00"), Status::Open), now).ends_with("OVERDUE"));
        assert!(!deadline_text(&t(1, Some("2026-11-03 11:00"), Status::Done), now).contains("OVERDUE"));
        assert_eq!(deadline_text(&t(1, None, Status::Open), now), "");
    }

    #[test]
    fn title_cleanup() {
        assert_eq!(clean_title("  a | b\n"), "a / b");
    }
}
