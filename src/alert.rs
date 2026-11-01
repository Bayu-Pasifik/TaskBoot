//! Fire a scheduler event: balloon, sound. The tray icon swap is driven by `main::tick`.

use crate::clock;
use crate::model::{Minute, Status, Task};
use crate::scheduler::Event;
use crate::tray;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
use windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONWARNING;

fn title_of(tasks: &[Task], id: u32) -> &str {
    tasks.iter().find(|t| t.id == id).map(|t| t.title.as_str()).unwrap_or("Task")
}

fn join_titles<'a>(it: impl Iterator<Item = &'a Task>) -> String {
    let mut out = String::new();
    for (i, t) in it.enumerate() {
        if i == 3 {
            out.push_str("; ...");
            break;
        }
        if i > 0 {
            out.push_str("; ");
        }
        out.push_str(&t.title);
    }
    out
}

fn time_left(d: Minute, now: Minute) -> String {
    let mins = d.0.saturating_sub(now.0);
    let mut s = String::from("Due in ");
    if mins >= 60 {
        s.push_str(&(mins / 60).to_string());
        s.push_str(" h ");
    }
    s.push_str(&(mins % 60).to_string());
    s.push_str(" min (");
    s.push_str(&clock::format(d));
    s.push(')');
    s
}

/// Show the alert for `ev`. Returns the task id to select when the balloon is clicked.
pub fn fire(hwnd: HWND, ev: &Event, tasks: &[Task], now: Minute) -> Option<u32> {
    match *ev {
        Event::Approaching(id) => {
            let body = tasks
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.deadline)
                .map(|d| time_left(d, now))
                .unwrap_or_default();
            tray::balloon(hwnd, title_of(tasks, id), &body, false);
            Some(id)
        }
        Event::Overdue(id) => {
            let mut title = String::from("Overdue: ");
            title.push_str(title_of(tasks, id));
            let mut body = String::from("Was due ");
            if let Some(d) = tasks.iter().find(|t| t.id == id).and_then(|t| t.deadline) {
                body.push_str(&clock::format(d));
            }
            tray::balloon(hwnd, &title, &body, true);
            beep();
            Some(id)
        }
        Event::DailySummary => {
            let open = tasks.iter().filter(|t| t.status == Status::Open && t.deadline.is_none());
            let n = open.clone().count();
            let mut title = n.to_string();
            title.push_str(if n == 1 { " open task" } else { " open tasks" });
            tray::balloon(hwnd, &title, &join_titles(open), false);
            None
        }
        Event::MissedSummary => {
            let od = tasks
                .iter()
                .filter(|t| t.status == Status::Open && matches!(t.deadline, Some(d) if d <= now));
            let n = od.clone().count();
            let mut title = n.to_string();
            title.push_str(if n == 1 { " overdue task" } else { " overdue tasks" });
            tray::balloon(hwnd, &title, &join_titles(od), true);
            beep();
            None
        }
    }
}

fn beep() {
    // SAFETY: MessageBeep has no preconditions.
    unsafe { MessageBeep(MB_ICONWARNING) };
}
