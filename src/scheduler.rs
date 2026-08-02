use chrono::Timelike;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub cron: String,
    #[serde(default)]
    pub action: ScheduledAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduledAction {
    #[default]
    CheckMissingFiles,
}

pub fn parse_cron(expr: &str) -> Result<croner::Cron, croner::errors::CronError> {
    croner::parser::CronParser::builder()
        .seconds(croner::parser::Seconds::Optional)
        .build()
        .parse(expr)
}

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

    #[test]
    fn parse_cron_seconds_optional() {
        assert!(parse_cron("0/30 * * * * *").is_ok());
        assert!(parse_cron("0 18 * * *").is_ok());
        assert!(parse_cron("not a cron").is_err());
    }
}
