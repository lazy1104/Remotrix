use chrono::{Datelike, Timelike};

pub fn parse_hhmm(s: &str) -> Option<(u8, u8)> {
    let (h, m) = s.split_once(':')?;
    let h: u8 = h.trim().parse().ok()?;
    let m: u8 = m.trim().parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some((h, m))
}

pub fn in_speed_window(start: &str, end: &str, now: &chrono::DateTime<chrono::Local>) -> bool {
    let (Some((sh, sm)), Some((eh, em))) = (parse_hhmm(start), parse_hhmm(end)) else {
        return false;
    };
    let start_min = sh as u32 * 60 + sm as u32;
    let end_min = eh as u32 * 60 + em as u32;
    let t = now.hour() * 60 + now.minute();
    if start_min == end_min {
        return true;
    }
    if start_min < end_min {
        t >= start_min && t < end_min
    } else {
        t >= start_min || t < end_min
    }
}

pub fn weekday_active(weekdays: &[u8], now: &chrono::DateTime<chrono::Local>) -> bool {
    weekdays.is_empty() || weekdays.contains(&(now.weekday().number_from_monday() as u8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Local, TimeZone};

    fn at(h: u32, m: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 1, 1, h, m, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn parse_hhmm_valid() {
        assert_eq!(parse_hhmm("23:00"), Some((23, 0)));
        assert_eq!(parse_hhmm("07:30"), Some((7, 30)));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("12:60"), None);
        assert_eq!(parse_hhmm("abc"), None);
    }

    #[test]
    fn window_same_start_end_is_always_inside() {
        assert!(in_speed_window("23:00", "23:00", &at(0, 0)));
        assert!(in_speed_window("23:00", "23:00", &at(23, 0)));
    }

    #[test]
    fn window_plain_range() {
        assert!(in_speed_window("09:00", "18:00", &at(9, 0)));
        assert!(in_speed_window("09:00", "18:00", &at(17, 59)));
        assert!(!in_speed_window("09:00", "18:00", &at(18, 0)));
        assert!(!in_speed_window("09:00", "18:00", &at(8, 59)));
    }

    #[test]
    fn window_crosses_midnight() {
        assert!(in_speed_window("23:00", "07:00", &at(23, 30)));
        assert!(in_speed_window("23:00", "07:00", &at(0, 0)));
        assert!(in_speed_window("23:00", "07:00", &at(6, 59)));
        assert!(!in_speed_window("23:00", "07:00", &at(7, 0)));
        assert!(!in_speed_window("23:00", "07:00", &at(12, 0)));
    }

    fn weekday_at(y: i32, m: u32, d: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, 12, 0, 0).single().unwrap()
    }

    #[test]
    fn weekday_empty_means_every_day() {
        assert!(weekday_active(&[], &weekday_at(2026, 1, 5)));
        assert!(weekday_active(&[], &weekday_at(2026, 1, 11)));
    }

    #[test]
    fn weekday_matches_selected() {
        assert!(weekday_active(&[1], &weekday_at(2026, 1, 5)));
        assert!(weekday_active(&[2, 3], &weekday_at(2026, 1, 6)));
        assert!(weekday_active(&[7], &weekday_at(2026, 1, 11)));
    }

    #[test]
    fn weekday_misses_unselected() {
        assert!(!weekday_active(&[1], &weekday_at(2026, 1, 6)));
        assert!(!weekday_active(&[2, 3], &weekday_at(2026, 1, 5)));
        assert!(!weekday_active(
            &[1, 2, 3, 4, 5, 6],
            &weekday_at(2026, 1, 11)
        ));
    }
}
