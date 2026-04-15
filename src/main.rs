use std::{thread::sleep, time::Duration};

use adhan::model::AdhanError;
use adhan::schedule::next_midnight_after;
use adhan::{
    build_prayer_queue, create_config, initialize_user_config_directory, new_timetable, play_adhan, read_config,
    AdhanCommands,
};

use chrono::{Datelike, Local};
use clap::Parser;
use salah::Event;

const CONFIGURATION_INIT_EXIT_CODE: i32 = 1;
const CONFIGURATION_CREATE_EXIT_CODE: i32 = 2;
const CONFIGURATION_READ_EXIT_CODE: i32 = 3;
const PLAYBACK_EXIT_CODE: i32 = 4;

fn initialise_logging() {
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    )
    .expect("initializing logger");
}

/// Unwraps `result`, returning the inner value on success.
/// On error, logs the error message and exits with the given code.
fn exit_on_error<T>(result: Result<T, AdhanError>, code: i32) -> T {
    result.unwrap_or_else(|err| {
        log::error!("{}", err);
        std::process::exit(code);
    })
}

/// Runs the main scheduler loop.
///
/// Loads the prayer timetable for the current day, works through each prayer
/// in chronological order (skipping any that have already passed), plays the
/// adhan at each prayer time, then sleeps until civil midnight before
/// reloading for the next day.
///
/// This function never returns — it loops indefinitely until the process is
/// killed.
fn run() -> ! {
    let parameters = exit_on_error(read_config(), CONFIGURATION_READ_EXIT_CODE);

    log::info!("Started Adhan!");

    'day: loop {
        let timetable = new_timetable(&parameters);
        let mut queue = build_prayer_queue(&timetable);

        // Drop any prayers whose time has already passed.
        let now = Local::now();
        queue.retain(|(time, _)| *time >= now);

        if queue.is_empty() {
            log::info!("All prayers for {} have already passed.", now.format("%A, %-d %B %Y"));
        } else {
            log::info!(
                "Loaded timetable for {}. {} prayer(s) remaining today.",
                now.format("%A, %-d %B %Y"),
                queue.len(),
            );
        }

        // Work through every prayer remaining today.
        while let Some((prayer_time, event)) = queue.pop_front() {
            let now = Local::now();
            let secs = prayer_time.signed_duration_since(now).num_seconds();

            let event_name = if now.weekday() == chrono::Weekday::Fri {
                event.friday_name()
            } else {
                event.name()
            };

            if secs > 0 {
                let hours = secs / 3600;
                let minutes = (secs % 3600) / 60;
                log::info!(
                    "Next: {} at {} ({}h {}m away) – sleeping",
                    event_name,
                    prayer_time.format("%H:%M"),
                    hours,
                    minutes,
                );
                sleep(Duration::from_secs(secs as u64));
            }

            log::info!("{} – playing adhan", event_name);
            exit_on_error(play_adhan(event), PLAYBACK_EXIT_CODE);
        }

        // All prayers done for today. Sleep until 00:00 of the next calendar
        // day, then rebuild the timetable.
        //
        // We deliberately do NOT use timetable.midnight(), which is Islamic
        // midnight (midpoint of the night), not the civil calendar boundary.
        let now = Local::now();
        let today = now.date_naive();
        let next_midnight = next_midnight_after(today);

        let secs_to_midnight = next_midnight.signed_duration_since(now).num_seconds();
        if secs_to_midnight > 0 {
            log::info!("All prayers complete. Sleeping until midnight for new day.");
            sleep(Duration::from_secs(secs_to_midnight as u64));
        }

        // std::thread::sleep only guarantees a *minimum* sleep duration — the
        // OS may wake us fractionally early. Spin in short bursts until the
        // calendar date has actually advanced before rebuilding, so we never
        // accidentally reload yesterday's timetable and get stuck in a loop.
        while Local::now().date_naive() == today {
            sleep(Duration::from_secs(1));
        }

        log::info!("Midnight reached – loading tomorrow's timetable.");
        continue 'day;
    }
}

fn main() {
    initialise_logging();

    exit_on_error(initialize_user_config_directory(), CONFIGURATION_INIT_EXIT_CODE);

    match AdhanCommands::parse() {
        AdhanCommands::Generate { method } => {
            exit_on_error(create_config(method), CONFIGURATION_CREATE_EXIT_CODE);
        }
        AdhanCommands::Test { use_fajr } => {
            let event = if use_fajr {
                Event::Prayer(salah::Prayer::Fajr)
            } else {
                Event::Prayer(salah::Prayer::Isha)
            };
            exit_on_error(play_adhan(event), PLAYBACK_EXIT_CODE);
        }
        AdhanCommands::Timetable => {
            let parameters = exit_on_error(read_config(), CONFIGURATION_READ_EXIT_CODE);
            let timetable = new_timetable(&parameters);
            println!("{}", timetable.display(&Local::now()));
        }
        AdhanCommands::Run => run(),
    }
}
