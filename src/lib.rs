pub mod model;

use std::{
    collections::VecDeque,
    fs::{DirBuilder, File},
    io::BufReader,
    path::PathBuf,
};

use chrono::{LocalResult, NaiveDate};

pub use model::{AdhanCommands, AdhanListSubcommand};
use model::{AdhanError, AdhanParameters, Method};
use rand::seq::SliceRandom;
use rodio::{cpal::traits::HostTrait, Decoder, Device, DeviceTrait, OutputStream, Sink};
use salah::{Coordinates, Event, Local, Prayer, Schedule, Times};

use crate::model::{AdhanAudioError, AdhanType};

static AUDIO_PATH: &str = "audio";
static SETTINGS_FILE: &str = "settings.yaml";

pub fn initialize_user_config_directory() -> Result<(), AdhanError> {
    if adhan_base_directory().is_ok_and(|dir| !dir.exists()) {
        let audio_path = &adhan_audio_directory()?;

        // Create the base audio directory.
        DirBuilder::new()
            .recursive(true)
            .create(audio_path)
            .map_err(AdhanError::IO)?;

        // Create expected subdirectories for different adhan types.
        DirBuilder::new()
            .recursive(true)
            .create(audio_path.join("fajr"))
            .map_err(AdhanError::IO)?;

        DirBuilder::new()
            .recursive(true)
            .create(audio_path.join("normal"))
            .map_err(AdhanError::IO)?;

        log::info!("Adhan program initialized!");
        log::info!("To configure:");
        log::info!("- Generate a configuration file using 'adhan generate <METHOD>'");
        log::info!("- Place Fajr adhan audio files at '{}/fajr'", audio_path.display());
        log::info!(
            "- Place standard adhan audio files at '{}/normal'",
            audio_path.display()
        );
    }

    Ok(())
}

pub fn adhan_base_directory() -> Result<PathBuf, AdhanError> {
    directories_next::ProjectDirs::from("", "", "adhan")
        .ok_or_else(|| AdhanError::Configuration("cannot generate configuration folder for 'adhan'".into()))
        .map(|project_dirs| project_dirs.config_dir().to_path_buf())
}

pub fn adhan_audio_directory() -> Result<PathBuf, AdhanError> {
    adhan_base_directory().map(|p| p.join(AUDIO_PATH))
}

pub fn read_config() -> Result<AdhanParameters, AdhanError> {
    let config_dir = adhan_base_directory()?;

    let config_path = config_dir.join(SETTINGS_FILE);
    let file = File::open(config_path).map_err(AdhanError::IO)?;

    serde_yaml::from_reader(file).map_err(AdhanError::Serialization)
}

pub fn create_config(method: Method) -> Result<(), AdhanError> {
    let config_dir = adhan_base_directory()?;

    let config_path = config_dir.join(SETTINGS_FILE);
    let file = File::create(config_path).map_err(AdhanError::IO)?;

    serde_yaml::to_writer(
        file,
        &AdhanParameters {
            coordinates: Coordinates::new(0.0, 0.0),
            parameters: method.parameters(),
        },
    )
    .map_err(AdhanError::Serialization)
}

pub fn play_adhan(prayer: Event, device: &str) -> Result<(), AdhanError> {
    let adhan_type = match prayer {
        Event::Qiyam | Event::Sunrise | Event::Restricted(_) => return Ok(()),
        Event::Prayer(Prayer::Fajr) => AdhanType::Fajr,
        _ => AdhanType::Normal,
    };

    let audio_config_path = adhan_audio_directory()?;

    assert!(
        std::fs::metadata(&audio_config_path).is_ok(),
        "Audio folder is not present. Please create one at {}.",
        audio_config_path.display(),
    );

    // Get a output stream handle to the default physical sound device

    let (_stream, stream_handle) = get_device(device)
        .map_or_else(OutputStream::try_default, |device| {
            OutputStream::try_from_device(&device)
        })
        .map_err(AdhanAudioError::Stream)
        .map_err(AdhanError::Audio)?;

    // Load a sound from a random audio file
    let audio_dir = std::fs::read_dir(audio_config_path.join({
        if adhan_type == AdhanType::Fajr {
            "fajr"
        } else {
            "normal"
        }
    }))
    .map_err(AdhanError::IO)?
    .filter_map(|f| f.ok())
    .filter_map(|f| {
        if f.file_type().is_ok_and(|t| t.is_file())
            && f
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3"))
        {
            Some(f)
        } else {
            None
        }
    })
    .collect::<Vec<_>>();

    let mut rng = rand::thread_rng();

    let audio_file_path = audio_dir
        .choose(&mut rng)
        .ok_or_else(|| AdhanError::Misc(String::from("no audio files available!")))?
        .path();
    let audio_file = std::fs::OpenOptions::new()
        .read(true)
        .open(audio_file_path)
        .map_err(AdhanError::IO)?;

    let file = BufReader::new(audio_file);

    // Decode that sound file into a source
    let source = Decoder::new(file)
        .map_err(AdhanAudioError::Decode)
        .map_err(AdhanError::Audio)?;

    let sink = Sink::try_new(&stream_handle)
        .map_err(AdhanAudioError::Playback)
        .map_err(AdhanError::Audio)?;

    // Add a dummy source of the sake of the example.
    sink.append(source);

    // The sound plays in a separate thread. This call will block the current thread until the sink
    // has finished playing all its queued sounds.
    sink.sleep_until_end();

    Ok(())
}

pub fn list_audio_devices() {
    let host = rodio::cpal::default_host();
    if let Ok(devices) = host.output_devices() {
        for (idx, device) in devices.flat_map(|device| device.name()).enumerate() {
            println!("{idx}: {device}");
        }
    }
}

pub fn list_audio_hosts() {
    for (idx, device) in rodio::cpal::available_hosts().iter().enumerate() {
        println!("{}: {}", idx, device.name());
    }
}

#[must_use]
pub fn new_timetable(parameters: &AdhanParameters) -> Times<Local> {
    Schedule::<Local>::now()
        .with_coordinates(parameters.coordinates())
        .with_parameters(parameters.parameters())
        .build()
        .unwrap_or_else(|err| {
            log::error!("Failed to calculate prayer times! - {err}");
            std::process::exit(1);
        })
}

/// Builds an ordered queue of the five daily prayers from a timetable.
/// Prayers are already in chronological order so no sorting is required.
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
/// This is the calendar midnight we sleep to at the end of each day. It is
/// deliberately *not* `timetable.midnight()`, which is Islamic midnight
/// (midpoint of the night between Maghrib and Fajr), not the civil boundary.
pub fn next_midnight_after(date: NaiveDate) -> chrono::DateTime<Local> {
    use chrono::TimeZone as _;
    let tomorrow = date.succ_opt().expect("date overflow computing next midnight");
    let naive_midnight = tomorrow
        .and_hms_opt(0, 0, 0)
        .expect("invalid time constructing local midnight");

    match Local.from_local_datetime(&naive_midnight) {
        LocalResult::Single(dt) => dt,
        // If midnight is ambiguous (e.g. due to offset changes), pick the earlier instant.
        LocalResult::Ambiguous(earlier, _later) => earlier,
        // If midnight does not exist in the local time zone on this date, fall back to
        // midnight in UTC for the same date, converted to Local.
        LocalResult::None => {
            let utc_midnight =
                chrono::DateTime::<chrono::Utc>::from_utc(naive_midnight, chrono::Utc);
            utc_midnight.with_timezone(&Local)
        }
    }
}

fn get_device(device_name: &str) -> Option<Device> {
    rodio::cpal::default_host()
        .output_devices()
        .into_iter()
        .flatten()
        .find_map(|dev| {
            dev.name()
                .map_or(None, |name| if name == device_name { Some(dev) } else { None })
        })
}

#[cfg(test)]
mod tests {
    use salah::{Coordinates, Parameters, TimeZone, Times};

    use super::*;

    /// Fixed coordinates and parameters used across all tests.
    /// London, MoonsightingCommittee — produces well-known, stable times.
    fn test_timetable() -> Times<Local> {
        // 2024-03-20 — spring equinox, straightforward prayer times.
        // We use Local so the returned DateTimes match what the real run loop
        // works with, making the retain comparisons meaningful.
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
        use salah::Prayer;
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
        // Use a time before Fajr — all five should survive.
        let before_fajr = timetable.fajr().clone() - chrono::Duration::hours(1);
        queue.retain(|(t, _)| *t > before_fajr);
        assert_eq!(queue.len(), 5);
    }

    #[test]
    fn retain_drops_all_prayers_when_all_are_in_the_past() {
        let timetable = test_timetable();
        let mut queue = build_prayer_queue(&timetable);
        // Use a time after Isha — none should survive.
        let after_isha = timetable.isha().clone() + chrono::Duration::hours(1);
        queue.retain(|(t, _)| *t > after_isha);
        assert!(queue.is_empty());
    }

    #[test]
    fn retain_keeps_only_future_prayers_mid_day() {
        use salah::Prayer;
        let timetable = test_timetable();
        let mut queue = build_prayer_queue(&timetable);
        // Simulate being between Asr and Maghrib — should keep Maghrib and Isha.
        let between_asr_and_maghrib = *timetable.asr() + chrono::Duration::minutes(30);
        queue.retain(|(t, _)| *t > between_asr_and_maghrib);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue[0].1, Event::Prayer(Prayer::Maghrib));
        assert_eq!(queue[1].1, Event::Prayer(Prayer::Isha));
    }

    #[test]
    fn retain_keeps_only_isha_when_between_maghrib_and_isha() {
        use salah::Prayer;
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
        // Timelike is in scope via chrono above
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
        use chrono::{NaiveDate, TimeZone};
        let today = NaiveDate::from_ymd_opt(2024, 3, 20).unwrap();
        let midnight = next_midnight_after(today);
        // Even 23:59:59 today must be before the returned midnight.
        let last_second_today = Local
            .from_local_datetime(&today.and_hms_opt(23, 59, 59).unwrap())
            .unwrap();
        assert!(midnight > last_second_today);
    }
}
