use std::collections::VecDeque;

use chrono::{LocalResult, NaiveDate};
use salah::{Event, Local, Prayer, Schedule, Times};

use crate::model::AdhanParameters;

/// Trait for types that respond to a prayer event firing.
///
/// The scheduler calls `on_prayer` once per handler for each prayer that
/// fires. Handlers are independent — a failure in one does not affect others.
/// Errors are logged by the caller; handlers should not exit the process.
///
/// The `event_name` parameter is the display name already resolved for the
/// current day (i.e. Friday Dhuhr is already passed as its Friday name
/// by the time `on_prayer` is called).
pub trait PrayerEventHandler {
    fn on_prayer(&self, event: &Event, event_name: &str) -> Result<(), crate::model::AdhanError>;
}

/// Builds a `Times<Local>` timetable for today using the given coordinates
/// and calculation parameters.
///
/// Panics (via `process::exit`) if the `salah` scheduler fails to produce a
/// valid timetable — this should never happen with well-formed coordinates and
/// parameters, but is treated as unrecoverable because the run loop cannot
/// proceed without a timetable.
#[must_use]
pub fn new_timetable(parameters: &AdhanParameters) -> Times<Local> {
    Schedule::<Local>::now()
        .with_coordinates(parameters.coordinates())
        .with_parameters(parameters.parameters())
        .build()
        .unwrap_or_else(|err| {
            log::error!("Failed to calculate prayer times: {err}");
            std::process::exit(1);
        })
}

/// Builds an ordered queue of the five daily prayers from a timetable.
///
/// The returned `VecDeque` is in chronological order (Fajr → Isha). No
/// filtering is applied here — callers are responsible for dropping prayers
/// that have already passed (e.g. via `retain`).
pub fn build_prayer_queue(timetable: &Times<Local>) -> VecDeque<(chrono::DateTime<Local>, Event)> {
    VecDeque::from([
        (timetable.fajr().clone(), Event::Prayer(Prayer::Fajr)),
        (timetable.dhuhr().clone(), Event::Prayer(Prayer::Dhuhr)),
        (timetable.asr().clone(), Event::Prayer(Prayer::Asr)),
        (timetable.maghrib().clone(), Event::Prayer(Prayer::Maghrib)),
        (timetable.isha().clone(), Event::Prayer(Prayer::Isha)),
    ])
}

/// Returns the `DateTime<Local>` representing 00:00:00 of the day after `date`.
///
/// This is the civil calendar midnight the run loop sleeps to at the end of
/// each day before rebuilding the timetable.
///
/// # Note
///
/// This is deliberately *not* `timetable.midnight()`, which is Islamic midnight
/// (the midpoint of the night between Maghrib and Fajr), not the civil
/// calendar boundary. Do not change this without understanding that distinction.
pub fn next_midnight_after(date: NaiveDate) -> chrono::DateTime<Local> {
    use chrono::TimeZone as _;

    let tomorrow = date.succ_opt().expect("date overflow computing next midnight");
    let naive_midnight = tomorrow
        .and_hms_opt(0, 0, 0)
        .expect("invalid time constructing local midnight");

    match Local.from_local_datetime(&naive_midnight) {
        LocalResult::Single(dt) => dt,
        // Midnight is ambiguous (e.g. a DST overlap) — use the earlier instant
        // so we never accidentally re-enter yesterday's date.
        LocalResult::Ambiguous(earlier, _later) => earlier,
        // Midnight does not exist in the local timezone on this date (e.g. a
        // DST gap). Fall back to UTC midnight converted to Local.
        LocalResult::None => {
            let utc_midnight = chrono::Utc.from_utc_datetime(&naive_midnight);
            utc_midnight.with_timezone(&Local)
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone as _;
    use salah::{Coordinates, Parameters, Times};

    use super::*;

    /// Fixed timetable used across all scheduling tests.
    ///
    /// 2024-03-20 (spring equinox), London, MoonsightingCommittee.
    /// This date and location produce stable, well-known prayer times that
    /// do not shift between runs.
    fn test_timetable() -> Times<Local> {
        let date = Local.with_ymd_and_hms(2024, 3, 20, 0, 0, 0).unwrap();
        let coords = Coordinates::new(51.5074, -0.1278); // London
        let params = Parameters::from_method(salah::Method::MoonsightingCommittee);
        Times::new(&date, &coords, &params)
    }

    // ── build_prayer_queue ────────────────────────────────────────────────────

    #[test]
    fn queue_contains_exactly_five_prayers() {
        let timetable = test_timetable();
        let queue = build_prayer_queue(&timetable);
        assert_eq!(queue.len(), 5);
    }

    #[test]
    fn queue_events_are_correct_prayers_in_order() {
        let timetable = test_timetable();
        let queue = build_prayer_queue(&timetable);
        let events: Vec<Event> = queue.into_iter().map(|(_, e)| e).collect();
        assert_eq!(
            events,
            vec![
                Event::Prayer(Prayer::Fajr),
                Event::Prayer(Prayer::Dhuhr),
                Event::Prayer(Prayer::Asr),
                Event::Prayer(Prayer::Maghrib),
                Event::Prayer(Prayer::Isha),
            ]
        );
    }

    #[test]
    fn queue_times_are_strictly_ascending() {
        let timetable = test_timetable();
        let queue = build_prayer_queue(&timetable);
        let times: Vec<_> = queue.into_iter().map(|(t, _)| t).collect();
        for window in times.windows(2) {
            assert!(
                window[0] < window[1],
                "times are not ascending: {:?} >= {:?}",
                window[0],
                window[1]
            );
        }
    }

    // ── retain (mid-day filtering) ────────────────────────────────────────────

    #[test]
    fn retain_keeps_all_prayers_when_all_are_in_the_future() {
        let timetable = test_timetable();
        let mut queue = build_prayer_queue(&timetable);
        let before_fajr = timetable.fajr().clone() - chrono::Duration::hours(1);
        queue.retain(|(t, _)| *t > before_fajr);
        assert_eq!(queue.len(), 5);
    }

    #[test]
    fn retain_drops_all_prayers_when_all_are_in_the_past() {
        let timetable = test_timetable();
        let mut queue = build_prayer_queue(&timetable);
        let after_isha = timetable.isha().clone() + chrono::Duration::hours(1);
        queue.retain(|(t, _)| *t > after_isha);
        assert!(queue.is_empty());
    }

    #[test]
    fn retain_keeps_only_future_prayers_mid_day() {
        let timetable = test_timetable();
        let mut queue = build_prayer_queue(&timetable);
        let between_asr_and_maghrib = *timetable.asr() + chrono::Duration::minutes(30);
        queue.retain(|(t, _)| *t > between_asr_and_maghrib);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue[0].1, Event::Prayer(Prayer::Maghrib));
        assert_eq!(queue[1].1, Event::Prayer(Prayer::Isha));
    }

    #[test]
    fn retain_keeps_only_isha_when_between_maghrib_and_isha() {
        let timetable = test_timetable();
        let mut queue = build_prayer_queue(&timetable);
        let between_maghrib_and_isha = *timetable.maghrib() + chrono::Duration::minutes(30);
        queue.retain(|(t, _)| *t > between_maghrib_and_isha);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].1, Event::Prayer(Prayer::Isha));
    }

    // ── next_midnight_after ───────────────────────────────────────────────────

    #[test]
    fn next_midnight_is_exactly_00_00_00_of_the_following_day() {
        use chrono::{Datelike, NaiveDate, Timelike};
        let today = NaiveDate::from_ymd_opt(2024, 3, 20).unwrap();
        let midnight = next_midnight_after(today);
        assert_eq!(midnight.date_naive().year(), 2024);
        assert_eq!(midnight.date_naive().month(), 3);
        assert_eq!(midnight.date_naive().day(), 21);
        assert_eq!(midnight.hour(), 0);
        assert_eq!(midnight.minute(), 0);
        assert_eq!(midnight.second(), 0);
    }

    #[test]
    fn next_midnight_rolls_over_month_boundary() {
        use chrono::{Datelike, NaiveDate};
        let last_day_of_march = NaiveDate::from_ymd_opt(2024, 3, 31).unwrap();
        let midnight = next_midnight_after(last_day_of_march);
        assert_eq!(midnight.date_naive().month(), 4);
        assert_eq!(midnight.date_naive().day(), 1);
    }

    #[test]
    fn next_midnight_rolls_over_year_boundary() {
        use chrono::{Datelike, NaiveDate};
        let new_years_eve = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let midnight = next_midnight_after(new_years_eve);
        assert_eq!(midnight.date_naive().year(), 2025);
        assert_eq!(midnight.date_naive().month(), 1);
        assert_eq!(midnight.date_naive().day(), 1);
    }

    #[test]
    fn next_midnight_is_strictly_after_any_time_today() {
        use chrono::NaiveDate;
        let today = NaiveDate::from_ymd_opt(2024, 3, 20).unwrap();
        let midnight = next_midnight_after(today);
        let last_second_today = Local
            .from_local_datetime(&today.and_hms_opt(23, 59, 59).unwrap())
            .unwrap();
        assert!(midnight > last_second_today);
    }
}
