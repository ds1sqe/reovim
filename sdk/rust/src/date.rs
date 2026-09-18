//! One instant of the Kernel clock as a calendar reads it.
//!
//! The clock counts nanoseconds and nothing else, so every side that
//! shows a time does the same arithmetic: `ls -l` and `stat` write
//! `YYYY-MM-DD HH:MM:SS` of it, `dmesg -T` and `-H` write the day and
//! the minute. The pieces are here and the wording is each one's own.

/// A Kernel instant in UTC. `weekday` counts Sunday 0, as `dmesg -T`
/// names the day.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Civil {
    pub year: u64,
    pub month: u64,
    pub day: u64,
    pub hour: u64,
    pub minute: u64,
    pub second: u64,
    pub weekday: u64,
}

/// The civil date of `nanos` of the Kernel clock, by the
/// civil-from-days arithmetic, which needs no table and no host.
///
/// ```
/// let at = reovim_sdk::date::civil(1_789_648_496_000_000_000);
/// assert_eq!((at.year, at.month, at.day), (2026, 9, 17));
/// assert_eq!((at.hour, at.minute, at.second), (12, 34, 56));
/// ```
#[must_use]
pub fn civil(nanos: u64) -> Civil {
    let seconds = nanos / 1_000_000_000;
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Counting the year from March, so the leap day is the last day.
    let shifted = days + 719_468;
    let (era, doe) = (shifted / 146_097, shifted % 146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = era * 400 + yoe + u64::from(month <= 2);
    Civil {
        year,
        month,
        day,
        hour: rest / 3600,
        minute: rest % 3600 / 60,
        second: rest % 60,
        // 1970-01-01 was a Thursday, the fourth day of the week.
        weekday: (days + 4) % 7,
    }
}

#[cfg(test)]
mod tests {
    use super::civil;

    /// The instants a calendar can be held against: the epoch, the
    /// second that has not come yet, 2026-09-17 12:34:56 UTC, and the
    /// two a leap year is read by.
    #[test]
    fn a_kernel_instant_is_the_civil_date_in_utc() {
        let at = |nanos| {
            let c = civil(nanos);
            ((c.year, c.month, c.day), (c.hour, c.minute, c.second))
        };
        assert_eq!(at(0), ((1970, 1, 1), (0, 0, 0)));
        assert_eq!(at(999_999_999), ((1970, 1, 1), (0, 0, 0)));
        assert_eq!(at(1_000_000_000), ((1970, 1, 1), (0, 0, 1)));
        assert_eq!(
            at(1_789_648_496_000_000_000),
            ((2026, 9, 17), (12, 34, 56))
        );
        // The leap day of a leap year, and the last second of it.
        assert_eq!(at(1_709_208_000_000_000_000), ((2024, 2, 29), (12, 0, 0)));
        assert_eq!(
            at(1_735_689_599_000_000_000),
            ((2024, 12, 31), (23, 59, 59))
        );
    }
}
