//! Fact clocks: TEXT `YYYY-MM-DD` / `YYYY-MM-DDTHH:MM:SSZ`. Envelope is Unix INTEGER.

use chrono::{DateTime, Utc};

pub(crate) const INSTANT_FMT: &str = "%Y-%m-%dT%H:%M:%SZ";

pub fn utc_iso(dt: DateTime<Utc>) -> String {
    dt.format(INSTANT_FMT).to_string()
}

pub fn utc_date() -> String {
    Utc::now().date_naive().to_string()
}

pub fn is_utc_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string() == s)
        .unwrap_or(false)
}

pub fn require_utc_date(s: &str, what: &str) -> anyhow::Result<()> {
    if is_utc_date(s) {
        Ok(())
    } else {
        anyhow::bail!("{what} must be UTC calendar day YYYY-MM-DD, got {s:?}")
    }
}

pub fn is_utc_instant(s: &str) -> bool {
    let Some(body) = s.strip_suffix('Z') else {
        return false;
    };
    chrono::NaiveDateTime::parse_from_str(body, "%Y-%m-%dT%H:%M:%S")
        .map(|dt| format!("{}Z", dt.format("%Y-%m-%dT%H:%M:%S")) == s)
        .unwrap_or(false)
}

pub fn require_utc_instant(s: &str, what: &str) -> anyhow::Result<()> {
    if is_utc_instant(s) {
        Ok(())
    } else {
        anyhow::bail!("{what} must be UTC instant YYYY-MM-DDTHH:MM:SSZ, got {s:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn utc_iso_is_zulu_second_resolution() {
        let dt = Utc.with_ymd_and_hms(2026, 9, 20, 16, 43, 5).unwrap();
        let s = utc_iso(dt);
        assert_eq!(s, "2026-09-20T16:43:05Z");
        assert!(is_utc_instant(&s));
        assert!(!is_utc_instant("2026-09-20T16:43:05+00:00"));
        assert!(!is_utc_instant("2026-09-20T16:43:05.000Z"));
    }

    #[test]
    fn accepts_calendar_day() {
        assert!(is_utc_date("2026-09-20"));
        assert!(!is_utc_date("2026/09/20"));
        assert!(!is_utc_date(""));
    }
}
