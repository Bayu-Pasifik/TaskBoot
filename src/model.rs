//! Pure data types. No Win32.

/// Local minutes since 1970-01-01 00:00.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Minute(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Open,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: u32,
    pub title: String,
    pub deadline: Option<Minute>,
    pub status: Status,
    pub snooze: Option<Minute>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// Minutes before the deadline for the "approaching" alert.
    pub lead: u32,
    /// Minutes between repeated overdue alerts.
    pub repeat: u32,
    /// Daily summary time (hour, minute).
    pub daily_summary: (u8, u8),
}

impl Default for Settings {
    fn default() -> Self {
        Settings { lead: 15, repeat: 30, daily_summary: (16, 0) }
    }
}

/// One line of tasks.txt. Anything not understood is kept verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Task(Task),
    Unparsed(String),
}

pub fn tasks_of(lines: &[Line]) -> Vec<Task> {
    lines
        .iter()
        .filter_map(|l| match l {
            Line::Task(t) => Some(t.clone()),
            Line::Unparsed(_) => None,
        })
        .collect()
}
