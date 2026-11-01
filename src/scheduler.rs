//! Pure alert rules (PRD A1-A5). Takes the current time as a parameter; no Win32.

use crate::clock;
use crate::model::{Minute, Settings, Status, Task};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Approaching(u32),
    Overdue(u32),
    DailySummary,
    MissedSummary,
}

#[derive(Debug, Clone, Default)]
pub struct AlertState {
    pub last_alerted: Vec<(u32, Minute)>,
    pub mute_until: Option<Minute>,
    pub last_daily: Option<Minute>,
    /// False until the first tick (login or resume) has run.
    pub started: bool,
}

impl AlertState {
    fn last(&self, id: u32) -> Option<Minute> {
        self.last_alerted.iter().find(|(i, _)| *i == id).map(|(_, m)| *m)
    }

    pub fn set_last(&mut self, id: u32, now: Minute) {
        match self.last_alerted.iter_mut().find(|(i, _)| *i == id) {
            Some(e) => e.1 = now,
            None => self.last_alerted.push((id, now)),
        }
    }

    pub fn is_muted(&self, now: Minute) -> bool {
        matches!(self.mute_until, Some(m) if m > now)
    }
}

fn active(t: &Task, now: Minute) -> bool {
    t.status == Status::Open && !matches!(t.snooze, Some(s) if s > now)
}

fn overdue(t: &Task, now: Minute) -> bool {
    t.status == Status::Open && matches!(t.deadline, Some(d) if d <= now)
}

fn repeat_of(s: &Settings) -> u64 {
    s.repeat.max(1) as u64
}

fn daily_at(s: &Settings, day: Minute) -> Minute {
    Minute(clock::day_start(day).0 + s.daily_summary.0 as u64 * 60 + s.daily_summary.1 as u64)
}

fn has_undated(tasks: &[Task]) -> bool {
    tasks.iter().any(|t| t.status == Status::Open && t.deadline.is_none())
}

pub fn due_events(tasks: &[Task], s: &Settings, st: &AlertState, now: Minute) -> Vec<Event> {
    let mut out = Vec::new();
    if st.is_muted(now) {
        return out;
    }
    let startup = !st.started;
    let missed = startup && tasks.iter().any(|t| active(t, now) && overdue(t, now));
    if missed {
        out.push(Event::MissedSummary);
    }
    for t in tasks {
        if !active(t, now) {
            continue;
        }
        let Some(d) = t.deadline else { continue };
        let last = st.last(t.id);
        if now >= d {
            if missed {
                continue;
            }
            let due = match last {
                None => true,
                Some(l) => l < d || now.0 >= l.0 + repeat_of(s),
            };
            if due {
                out.push(Event::Overdue(t.id));
            }
        } else {
            let from = Minute(d.0.saturating_sub(s.lead as u64));
            if s.lead > 0 && now >= from && !matches!(last, Some(l) if l >= from) {
                out.push(Event::Approaching(t.id));
            }
        }
    }
    if has_undated(tasks) {
        let at = daily_at(s, now);
        let scheduled = now >= at && !matches!(st.last_daily, Some(l) if l >= at);
        if startup || scheduled {
            out.push(Event::DailySummary);
        }
    }
    out
}

/// Apply the side effects of firing `ev` to the alert state.
pub fn record(st: &mut AlertState, ev: &Event, tasks: &[Task], now: Minute) {
    match ev {
        Event::Approaching(id) | Event::Overdue(id) => st.set_last(*id, now),
        Event::DailySummary => st.last_daily = Some(now),
        Event::MissedSummary => {
            for t in tasks.iter().filter(|t| active(t, now) && overdue(t, now)) {
                st.set_last(t.id, now);
            }
        }
    }
}

/// Next time the scheduler needs to run again. Always > `now`, at most `now + 60`.
pub fn next_wake(tasks: &[Task], s: &Settings, st: &AlertState, now: Minute) -> Minute {
    let cap = Minute(now.0 + 60);
    let mut best = cap;
    let mut consider = |m: Minute| {
        if m > now && m < best {
            best = m;
        }
    };
    if st.is_muted(now) {
        if let Some(m) = st.mute_until {
            consider(m);
        }
        return best;
    }
    for t in tasks.iter().filter(|t| t.status == Status::Open) {
        if let Some(sn) = t.snooze {
            consider(sn);
        }
        let Some(d) = t.deadline else { continue };
        let last = st.last(t.id);
        if s.lead > 0 {
            let from = Minute(d.0.saturating_sub(s.lead as u64));
            if !matches!(last, Some(l) if l >= from) {
                consider(from);
            }
        }
        consider(d);
        if let Some(l) = last {
            if l >= d {
                consider(Minute(l.0 + repeat_of(s)));
            }
        }
    }
    if has_undated(tasks) {
        let today = daily_at(s, now);
        consider(today);
        consider(Minute(today.0 + 1440));
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(s: &str) -> Minute {
        clock::parse(s).unwrap()
    }

    fn task(id: u32, deadline: Option<&str>) -> Task {
        Task { id, title: "t".into(), deadline: deadline.map(m), status: Status::Open, snooze: None }
    }

    fn st() -> AlertState {
        AlertState { started: true, ..Default::default() }
    }

    fn s() -> Settings {
        Settings { lead: 15, repeat: 30, daily_summary: (16, 0) }
    }

    #[test]
    fn approaching_window() {
        let t = [task(1, Some("2026-11-03 17:00"))];
        assert!(due_events(&t, &s(), &st(), m("2026-11-03 16:44")).is_empty());
        assert_eq!(due_events(&t, &s(), &st(), m("2026-11-03 16:45")), [Event::Approaching(1)]);
        assert_eq!(due_events(&t, &s(), &st(), m("2026-11-03 16:59")), [Event::Approaching(1)]);
    }

    #[test]
    fn approaching_fires_once() {
        let t = [task(1, Some("2026-11-03 17:00"))];
        let mut state = st();
        state.set_last(1, m("2026-11-03 16:45"));
        assert!(due_events(&t, &s(), &state, m("2026-11-03 16:50")).is_empty());
    }

    #[test]
    fn overdue_at_deadline_after_approaching() {
        let t = [task(1, Some("2026-11-03 17:00"))];
        let mut state = st();
        state.set_last(1, m("2026-11-03 16:45"));
        assert_eq!(due_events(&t, &s(), &state, m("2026-11-03 17:00")), [Event::Overdue(1)]);
    }

    #[test]
    fn overdue_repeats_every_interval() {
        let t = [task(1, Some("2026-11-03 17:00"))];
        let mut state = st();
        state.set_last(1, m("2026-11-03 17:00"));
        assert!(due_events(&t, &s(), &state, m("2026-11-03 17:29")).is_empty());
        assert_eq!(due_events(&t, &s(), &state, m("2026-11-03 17:30")), [Event::Overdue(1)]);
    }

    #[test]
    fn done_tasks_never_alert() {
        let mut t = task(1, Some("2026-11-03 17:00"));
        t.status = Status::Done;
        assert!(due_events(&[t], &s(), &st(), m("2026-11-03 18:00")).is_empty());
    }

    #[test]
    fn snooze_blocks_then_expires() {
        let mut t = task(1, Some("2026-11-03 17:00"));
        t.snooze = Some(m("2026-11-03 18:00"));
        let mut state = st();
        state.set_last(1, m("2026-11-03 17:00"));
        assert!(due_events(&[t.clone()], &s(), &state, m("2026-11-03 17:59")).is_empty());
        assert_eq!(due_events(&[t], &s(), &state, m("2026-11-03 18:00")), [Event::Overdue(1)]);
    }

    #[test]
    fn mute_blocks_everything() {
        let t = [task(1, Some("2026-11-03 17:00")), task(2, None)];
        let mut state = st();
        state.mute_until = Some(m("2026-11-03 19:00"));
        assert!(due_events(&t, &s(), &state, m("2026-11-03 18:00")).is_empty());
        assert!(!due_events(&t, &s(), &state, m("2026-11-03 19:00")).is_empty());
    }

    #[test]
    fn no_deadline_tasks_only_daily() {
        let t = [task(1, None)];
        assert!(due_events(&t, &s(), &st(), m("2026-11-03 15:59")).is_empty());
        assert_eq!(due_events(&t, &s(), &st(), m("2026-11-03 16:00")), [Event::DailySummary]);
    }

    #[test]
    fn daily_once_per_day() {
        let t = [task(1, None)];
        let mut state = st();
        record(&mut state, &Event::DailySummary, &t, m("2026-11-03 16:00"));
        assert!(due_events(&t, &s(), &state, m("2026-11-03 20:00")).is_empty());
        assert_eq!(due_events(&t, &s(), &state, m("2026-11-04 16:00")), [Event::DailySummary]);
    }

    #[test]
    fn daily_skipped_when_no_open_undated() {
        let mut t = task(1, None);
        t.status = Status::Done;
        assert!(due_events(&[t], &s(), &st(), m("2026-11-03 16:00")).is_empty());
    }

    #[test]
    fn startup_summary_replaces_overdue_alerts() {
        let t = [task(1, Some("2026-11-03 10:00")), task(2, Some("2026-11-03 11:00")), task(3, None)];
        let state = AlertState::default();
        let ev = due_events(&t, &s(), &state, m("2026-11-03 12:00"));
        assert_eq!(ev, [Event::MissedSummary, Event::DailySummary]);
    }

    #[test]
    fn startup_without_overdue_has_no_missed_summary() {
        let t = [task(1, Some("2026-11-03 18:00"))];
        assert!(due_events(&t, &s(), &AlertState::default(), m("2026-11-03 12:00")).is_empty());
    }

    #[test]
    fn missed_record_marks_overdue_tasks() {
        let t = [task(1, Some("2026-11-03 10:00"))];
        let mut state = AlertState::default();
        let now = m("2026-11-03 12:00");
        for e in due_events(&t, &s(), &state, now) {
            record(&mut state, &e, &t, now);
        }
        state.started = true;
        assert!(due_events(&t, &s(), &state, m("2026-11-03 12:10")).is_empty());
        assert_eq!(due_events(&t, &s(), &state, m("2026-11-03 12:30")), [Event::Overdue(1)]);
    }

    #[test]
    fn next_wake_never_in_past_and_capped() {
        let now = m("2026-11-03 12:00");
        assert_eq!(next_wake(&[], &s(), &st(), now), Minute(now.0 + 60));
        let t = [task(1, Some("2026-11-03 12:30"))];
        // approaching at 12:15
        assert_eq!(next_wake(&t, &s(), &st(), now), m("2026-11-03 12:15"));
        // past deadlines do not produce past wake times
        let t = [task(1, Some("2026-11-03 09:00"))];
        assert!(next_wake(&t, &s(), &st(), now) > now);
    }

    #[test]
    fn next_wake_table() {
        let now = m("2026-11-03 12:00");
        let mut state = st();
        // overdue and already alerted -> repeat time
        let t = [task(1, Some("2026-11-03 11:50"))];
        state.set_last(1, m("2026-11-03 11:50"));
        assert_eq!(next_wake(&t, &s(), &state, now), m("2026-11-03 12:20"));
        // snooze expiry
        let mut sn = task(2, Some("2026-11-03 11:00"));
        sn.snooze = Some(m("2026-11-03 12:10"));
        assert_eq!(next_wake(&[sn], &s(), &st(), now), m("2026-11-03 12:10"));
        // mute end
        let mut ms = st();
        ms.mute_until = Some(m("2026-11-03 12:20"));
        assert_eq!(next_wake(&t, &s(), &ms, now), m("2026-11-03 12:20"));
        // daily summary within the hour
        let u = [task(3, None)];
        assert_eq!(next_wake(&u, &s(), &st(), m("2026-11-03 15:30")), m("2026-11-03 16:00"));
        // after today's daily time: next is tomorrow, but capped at +60
        assert_eq!(next_wake(&u, &s(), &st(), m("2026-11-03 17:00")), m("2026-11-03 18:00"));
    }

    #[test]
    fn zero_repeat_does_not_spin() {
        let mut set = s();
        set.repeat = 0;
        let t = [task(1, Some("2026-11-03 11:00"))];
        let mut state = st();
        state.set_last(1, m("2026-11-03 12:00"));
        let now = m("2026-11-03 12:00");
        assert!(due_events(&t, &set, &state, now).is_empty());
        assert!(next_wake(&t, &set, &state, now) > now);
    }
}
