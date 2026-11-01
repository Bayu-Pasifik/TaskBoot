//! Pure date/time maths on `Minute`. Only `now_local` touches Win32.

use crate::model::Minute;

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: u64, m: u64) -> u64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
    }
}

/// Days since 1970-01-01 for a civil date (year >= 1970).
pub fn days_from_civil(y: u64, m: u64, d: u64) -> u64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    // 719468 = days from 0000-03-01 to 1970-01-01
    era * 146097 + doe - 719468
}

/// Inverse of `days_from_civil`.
pub fn civil_from_days(z: u64) -> (u64, u64, u64) {
    let z = z + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn from_parts(y: u64, mo: u64, d: u64, h: u64, mi: u64) -> Option<Minute> {
    if y < 1970 || y > 9999 || !(1..=12).contains(&mo) || h > 23 || mi > 59 {
        return None;
    }
    if d < 1 || d > days_in_month(y, mo) {
        return None;
    }
    Some(Minute(days_from_civil(y, mo, d) * 1440 + h * 60 + mi))
}

fn num(s: &str, len: usize) -> Option<u64> {
    if s.len() != len || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Parse "YYYY-MM-DD HH:MM".
pub fn parse(s: &str) -> Option<Minute> {
    let s = s.trim();
    let (date, time) = s.split_once(' ')?;
    let mut d = date.split('-');
    let (y, mo, da) = (d.next()?, d.next()?, d.next()?);
    if d.next().is_some() {
        return None;
    }
    let (h, mi) = parse_hm(time.trim())?;
    from_parts(num(y, 4)?, num(mo, 2)?, num(da, 2)?, h as u64, mi as u64)
}

/// Parse "HH:MM".
pub fn parse_hm(s: &str) -> Option<(u8, u8)> {
    let (h, m) = s.split_once(':')?;
    let (h, m) = (num(h, 2)?, num(m, 2)?);
    if h > 23 || m > 59 {
        return None;
    }
    Some((h as u8, m as u8))
}

fn push2(out: &mut String, n: u64) {
    out.push((b'0' + (n / 10 % 10) as u8) as char);
    out.push((b'0' + (n % 10) as u8) as char);
}

/// Format as "YYYY-MM-DD HH:MM".
pub fn format(m: Minute) -> String {
    let days = m.0 / 1440;
    let rem = m.0 % 1440;
    let (y, mo, d) = civil_from_days(days);
    let mut out = String::with_capacity(16);
    push2(&mut out, y / 100);
    push2(&mut out, y % 100);
    out.push('-');
    push2(&mut out, mo);
    out.push('-');
    push2(&mut out, d);
    out.push(' ');
    push2(&mut out, rem / 60);
    out.push(':');
    push2(&mut out, rem % 60);
    out
}

/// Minute at the start (00:00) of the day containing `m`.
pub fn day_start(m: Minute) -> Minute {
    Minute(m.0 / 1440 * 1440)
}

/// Current local time, minute resolution.
pub fn now_local() -> Minute {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: SYSTEMTIME is plain data; GetLocalTime fills it.
    let st: SYSTEMTIME = unsafe {
        let mut st = core::mem::zeroed();
        GetLocalTime(&mut st);
        st
    };
    from_parts(st.wYear as u64, st.wMonth as u64, st.wDay as u64, st.wHour as u64, st.wMinute as u64)
        .unwrap_or(Minute(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch() {
        assert_eq!(parse("1970-01-01 00:00"), Some(Minute(0)));
        assert_eq!(parse("1970-01-02 00:01"), Some(Minute(1441)));
    }

    #[test]
    fn known_date() {
        // 2026-11-03 is day 20760 since epoch.
        assert_eq!(days_from_civil(2026, 11, 3), 20760);
        assert_eq!(parse("2026-11-03 17:00"), Some(Minute(20760 * 1440 + 17 * 60)));
    }

    #[test]
    fn round_trip_every_day_of_range() {
        for d in (0..60000).step_by(7) {
            let (y, m, da) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, da), d);
        }
        let m = parse("2024-02-29 23:59").unwrap();
        assert_eq!(format(m), "2024-02-29 23:59");
    }

    #[test]
    fn leap_years() {
        assert!(parse("2024-02-29 00:00").is_some());
        assert!(parse("2000-02-29 00:00").is_some());
        assert!(parse("2100-02-29 00:00").is_none());
        assert!(parse("2023-02-29 00:00").is_none());
        // Leap day adds exactly one day.
        let a = parse("2024-02-28 12:00").unwrap();
        let b = parse("2024-03-01 12:00").unwrap();
        assert_eq!(b.0 - a.0, 2 * 1440);
        let a = parse("2023-02-28 12:00").unwrap();
        let b = parse("2023-03-01 12:00").unwrap();
        assert_eq!(b.0 - a.0, 1440);
    }

    #[test]
    fn month_end() {
        assert!(parse("2026-04-30 10:00").is_some());
        assert!(parse("2026-04-31 10:00").is_none());
        assert!(parse("2026-12-31 23:59").is_some());
        let a = parse("2026-12-31 23:59").unwrap();
        let b = parse("2027-01-01 00:00").unwrap();
        assert_eq!(b.0 - a.0, 1);
    }

    #[test]
    fn rejects_garbage() {
        for s in ["", "2026-11-03", "2026-13-01 10:00", "2026-11-03 24:00", "2026-11-03 10:60",
                  "26-11-03 10:00", "2026-1-03 10:00", "abc", "1969-12-31 23:59", "2026-11-03 1:00"] {
            assert!(parse(s).is_none(), "{s}");
        }
    }

    #[test]
    fn hm() {
        assert_eq!(parse_hm("16:00"), Some((16, 0)));
        assert_eq!(parse_hm("24:00"), None);
        assert_eq!(parse_hm("9:00"), None);
    }

    #[test]
    fn day_start_works() {
        let m = parse("2026-11-03 17:45").unwrap();
        assert_eq!(format(day_start(m)), "2026-11-03 00:00");
    }
}
