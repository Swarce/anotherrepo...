//! The server clock. Everything time-based (events, stamina, login bonus) reads this, so a
//! simulated-clock run can fast-forward the server without touching the device clock.

use std::sync::atomic::{AtomicI64, Ordering};

static OFFSET_SECS: AtomicI64 = AtomicI64::new(0);

/// Seconds since the Unix epoch, including any simulated offset.
pub fn now() -> i64 {
    crate::log::unix_ms() as i64 / 1000 + OFFSET_SECS.load(Ordering::Relaxed)
}

pub fn set_offset(secs: i64) {
    OFFSET_SECS.store(secs, Ordering::Relaxed);
}

pub fn offset() -> i64 {
    OFFSET_SECS.load(Ordering::Relaxed)
}

/// Server timezone for the game's day boundaries (the game ran on JST, UTC+9).
pub const TZ_OFFSET_SECS: i64 = 9 * 3600;

/// Civil date/time (in the server timezone) for a Unix time: (y, m, d, hh, mm, ss).
pub fn civil(t: i64) -> (i64, u32, u32, u32, u32, u32) {
    let t = t + TZ_OFFSET_SECS;
    let days = t.div_euclid(86400);
    let secs = t.rem_euclid(86400);
    // Howard Hinnant's days_from_civil inverse
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        m,
        d,
        (secs / 3600) as u32,
        ((secs / 60) % 60) as u32,
        (secs % 60) as u32,
    )
}

/// `yyyy/MM/dd HH:mm:ss` in the server timezone.
pub fn format(t: i64) -> String {
    let (y, mo, d, h, mi, s) = civil(t);
    format!("{y:04}/{mo:02}/{d:02} {h:02}:{mi:02}:{s:02}")
}

/// `yyyyMMddHHmmss` as used in request headers.
pub fn compact(t: i64) -> String {
    let (y, mo, d, h, mi, s) = civil(t);
    format!("{y:04}{mo:02}{d:02}{h:02}{mi:02}{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn civil_known_dates() {
        // 2021-01-01T00:00:00Z == 2021/01/01 09:00:00 JST
        assert_eq!(format(1609459200), "2021/01/01 09:00:00");
        // leap day
        assert_eq!(format(951782400 - 9 * 3600), "2000/02/29 00:00:00");
        assert_eq!(compact(0), "19700101090000");
    }
}
