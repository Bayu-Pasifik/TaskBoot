//! tasks.txt parsing, serialization and atomic persistence. No Win32.

use crate::clock;
use crate::model::{Line, Settings, Status, Task};
use crate::win::{Error, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const DEFAULT: &str = include_str!("../assets/default_tasks.txt");

/// mtime (ns since epoch) of tasks.txt as last loaded or saved by us.
static LAST_MTIME: AtomicU64 = AtomicU64::new(0);

pub fn path() -> PathBuf {
    match std::env::current_exe() {
        Ok(p) => p.with_file_name("tasks.txt"),
        Err(_) => PathBuf::from("tasks.txt"),
    }
}

fn parse_task(line: &str, id: u32) -> Option<Task> {
    let t = line.trim();
    let (status, rest) = if let Some(r) = t.strip_prefix("[ ]") {
        (Status::Open, r)
    } else if let Some(r) = t.strip_prefix("[x]").or_else(|| t.strip_prefix("[X]")) {
        (Status::Done, r)
    } else {
        return None;
    };
    let parts: Vec<&str> = rest.split('|').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let d = parts[0].trim();
    let deadline = if d.is_empty() { None } else { Some(clock::parse(d)?) };
    let title = parts[1].trim();
    if title.is_empty() {
        return None;
    }
    let snooze = match parts.get(2) {
        None => None,
        Some(s) => Some(clock::parse(s.trim().strip_prefix("snooze=")?)?),
    };
    Some(Task { id, title: title.to_string(), deadline, status, snooze })
}

fn parse_setting(line: &str, s: &mut Settings) {
    let Some((k, v)) = line.trim().split_once('=') else { return };
    let (k, v) = (k.trim(), v.trim());
    match k {
        "lead_minutes" => {
            if let Ok(n) = v.parse::<u32>() {
                s.lead = n;
            }
        }
        "repeat_minutes" => {
            if let Ok(n) = v.parse::<u32>() {
                s.repeat = n;
            }
        }
        "daily_summary" => {
            if let Some(hm) = clock::parse_hm(v) {
                s.daily_summary = hm;
            }
        }
        _ => {}
    }
}

/// Never fails: bad lines are kept verbatim as `Line::Unparsed`.
pub fn parse(text: &str) -> (Settings, Vec<Line>) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut settings = Settings::default();
    let mut lines = Vec::new();
    let mut next_id = 1u32;
    for raw in text.lines() {
        if let Some(t) = parse_task(raw, next_id) {
            next_id += 1;
            lines.push(Line::Task(t));
        } else {
            if !raw.trim_start().starts_with('#') {
                parse_setting(raw, &mut settings);
            }
            lines.push(Line::Unparsed(raw.to_string()));
        }
    }
    (settings, lines)
}

fn task_line(t: &Task) -> String {
    let mut s = String::from(if t.status == Status::Done { "[x] " } else { "[ ] " });
    if let Some(d) = t.deadline {
        s.push_str(&clock::format(d));
        s.push(' ');
    }
    s.push_str("| ");
    s.push_str(&t.title);
    if let Some(sn) = t.snooze {
        s.push_str(" | snooze=");
        s.push_str(&clock::format(sn));
    }
    s
}

pub fn serialize(lines: &[Line]) -> String {
    let mut out = String::new();
    for l in lines {
        match l {
            Line::Task(t) => out.push_str(&task_line(t)),
            Line::Unparsed(s) => out.push_str(s),
        }
        out.push('\n');
    }
    out
}

fn mtime_ns(p: &Path) -> Option<u64> {
    let m = std::fs::metadata(p).ok()?.modified().ok()?;
    let d = m.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(d.as_nanos() as u64)
}

pub fn load_from(p: &Path) -> Result<(Settings, Vec<Line>)> {
    if !p.exists() {
        std::fs::write(p, DEFAULT).map_err(|e| Error::Io(e.kind()))?;
    }
    let text = std::fs::read_to_string(p).map_err(|e| Error::Io(e.kind()))?;
    LAST_MTIME.store(mtime_ns(p).unwrap_or(0), Ordering::Relaxed);
    Ok(parse(&text))
}

pub fn load() -> Result<(Settings, Vec<Line>)> {
    load_from(&path())
}

/// Atomic: write `<file>.tmp`, flush to disk, rename over the target.
pub fn save_to(p: &Path, lines: &[Line]) -> Result<()> {
    use std::io::Write;
    let mut tmp = p.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let io = |e: std::io::Error| Error::Io(e.kind());
    let mut f = std::fs::File::create(&tmp).map_err(io)?;
    f.write_all(serialize(lines).as_bytes()).map_err(io)?;
    f.sync_all().map_err(io)?;
    drop(f);
    std::fs::rename(&tmp, p).map_err(io)?;
    LAST_MTIME.store(mtime_ns(p).unwrap_or(0), Ordering::Relaxed);
    Ok(())
}

pub fn save(lines: &[Line]) -> Result<()> {
    save_to(&path(), lines)
}

/// True when tasks.txt was modified by someone else since our last load/save.
pub fn changed_on_disk() -> bool {
    match mtime_ns(&path()) {
        Some(m) => m != LAST_MTIME.load(Ordering::Relaxed),
        None => false,
    }
}

/// Minute helper used by tests.
#[cfg(test)]
pub fn minute(s: &str) -> Option<crate::model::Minute> {
    clock::parse(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tasks_of;

    const SAMPLE: &str = "\
# comment
[settings]
lead_minutes=15
repeat_minutes=30
daily_summary=16:00

[ ] 2026-11-03 17:00 | Send weekly report
[ ] 2026-11-04 09:00 | Review PR #42 | snooze=2026-11-04 10:00
[x] 2026-11-01 12:00 | Book flight
[ ] | Clean up inbox
";

    #[test]
    fn round_trip_byte_identical() {
        let (_, lines) = parse(SAMPLE);
        assert_eq!(serialize(&lines), SAMPLE);
    }

    #[test]
    fn parses_fields() {
        let (s, lines) = parse(SAMPLE);
        assert_eq!(s, Settings { lead: 15, repeat: 30, daily_summary: (16, 0) });
        let t = tasks_of(&lines);
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].id, 1);
        assert_eq!(t[0].title, "Send weekly report");
        assert_eq!(t[0].deadline, minute("2026-11-03 17:00"));
        assert_eq!(t[1].snooze, minute("2026-11-04 10:00"));
        assert_eq!(t[2].status, Status::Done);
        assert_eq!(t[3].deadline, None);
        assert_eq!(t[3].title, "Clean up inbox");
    }

    #[test]
    fn bad_lines_survive() {
        let text = "[ ] 2026-13-45 99:99 | bad date\nrandom junk\n[ ] 2026-11-03 17:00 |\n[ ] | ok\n[ ] | a | b | c\n[ ] | t | snooze=nope\n";
        let (_, lines) = parse(text);
        assert_eq!(tasks_of(&lines).len(), 1);
        assert_eq!(serialize(&lines), text);
    }

    #[test]
    fn bom_is_stripped() {
        let (_, lines) = parse("\u{feff}[ ] | with bom\n");
        assert_eq!(tasks_of(&lines).len(), 1);
    }

    #[test]
    fn missing_deadline_ok() {
        let (_, lines) = parse("[ ] | no deadline");
        let t = tasks_of(&lines);
        assert_eq!(t[0].deadline, None);
        assert_eq!(t[0].snooze, None);
    }

    #[test]
    fn settings_defaults_and_overrides() {
        let (s, _) = parse("[ ] | x");
        assert_eq!(s, Settings::default());
        let (s, _) = parse("lead_minutes=5\nrepeat_minutes=abc\ndaily_summary=09:30\n");
        assert_eq!(s.lead, 5);
        assert_eq!(s.repeat, 30);
        assert_eq!(s.daily_summary, (9, 30));
        let (s, _) = parse("# lead_minutes=99\n");
        assert_eq!(s.lead, 15);
    }

    #[test]
    fn snooze_round_trip() {
        let text = "[ ] 2026-11-04 09:00 | T | snooze=2026-11-04 10:00\n";
        let (_, lines) = parse(text);
        assert_eq!(serialize(&lines), text);
    }

    #[test]
    fn missing_file_creates_default_then_saves_atomically() {
        let dir = std::env::temp_dir().join("taskboot_store_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("tasks.txt");
        let (_, lines) = load_from(&p).unwrap();
        assert!(p.exists());
        assert_eq!(tasks_of(&lines).len(), 1);
        let mut lines = lines;
        lines.push(Line::Task(Task {
            id: 9, title: "new".into(), deadline: minute("2026-11-03 17:00"),
            status: Status::Open, snooze: None,
        }));
        save_to(&p, &lines).unwrap();
        assert!(!dir.join("tasks.txt.tmp").exists());
        let (_, again) = load_from(&p).unwrap();
        assert_eq!(tasks_of(&again).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
