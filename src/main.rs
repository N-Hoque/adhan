use std::{thread::sleep, time::Duration};

use adhan::{
    build_prayer_queue, create_config, initialize_user_config_directory, new_timetable, play_adhan, read_config,
    AdhanCommands,
};

use chrono::{Datelike, Local};
use clap::Parser;
use salah::{Event, Prayer};

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

fn main() {
    initialise_logging();

    if let Err(err) = initialize_user_config_directory() {
        log::error!("{}", err);
        std::process::exit(CONFIGURATION_INIT_EXIT_CODE);
    }

    match AdhanCommands::parse() {
        AdhanCommands::Generate { method } => {
            if let Err(err) = create_config(method) {
                log::error!("{}", err);
                std::process::exit(CONFIGURATION_CREATE_EXIT_CODE);
            }
        }
        AdhanCommands::Test { audio_device, use_fajr } => {
            if let Err(err) = play_adhan(
                if use_fajr {
                    Event::Prayer(Prayer::Fajr)
                } else {
                    Event::Prayer(Prayer::Isha)
                },
                &audio_device,
            ) {
                log::error!("{}", err);
                std::process::exit(PLAYBACK_EXIT_CODE);
            }
        }
        AdhanCommands::Timetable => match read_config() {
            Err(err) => {
                log::error!("{}", err);
                std::process::exit(CONFIGURATION_READ_EXIT_CODE);
            }
            Ok(parameters) => {
                let timetable = new_timetable(&parameters);
                let current_time = Local::now();
                println!("{}", timetable.display(&current_time));
            }
        },
        AdhanCommands::Run { audio_device } => match read_config() {
            Err(err) => {
                log::error!("{}", err);
                std::process::exit(CONFIGURATION_READ_EXIT_CODE);
            }
            Ok(parameters) => {
                log::info!("Started Adhan!");

                'day: loop {
                    let timetable = new_timetable(&parameters);
                    let mut queue = build_prayer_queue(&timetable);

                    // Drop any prayers that have already passed.
                    let now = Local::now();
                    queue.retain(|(time, _)| *time >= now);

                    if queue.is_empty() {
                        log::info!("All prayers for {} have already passed.", now.format("%A, %-d %B %Y"));
                    } else {
                        log::info!(
                            "Loaded timetable for {}. {} prayer(s) remaining today.",
                            now.format("%A, %-d %B %Y"),
                            queue.len()
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
                        if let Err(err) = play_adhan(event, &audio_device) {
                            log::error!("{}", err);
                            std::process::exit(PLAYBACK_EXIT_CODE);
                        }
                    }

                    // All prayers done (or already past on startup).
                    // Sleep until 00:00 of the next calendar day, then
                    // rebuild the timetable. We deliberately do NOT use
                    // timetable.midnight(), which is Islamic midnight
                    // (midpoint of the night), not the calendar boundary.
                    let now = Local::now();
                    let today = now.date_naive();
                    let next_midnight = adhan::next_midnight_after(today);

                    let secs_to_midnight = next_midnight.signed_duration_since(now).num_seconds();
                    if secs_to_midnight > 0 {
                        log::info!("All prayers complete. Sleeping until midnight for new day.");
                        sleep(Duration::from_secs(secs_to_midnight as u64));
                    }

                    // std::thread::sleep only guarantees a *minimum* sleep
                    // duration — the OS may wake us fractionally early. Spin
                    // in short bursts until the calendar date has actually
                    // advanced before rebuilding, so we never accidentally
                    // reload yesterday's timetable and get stuck in a loop.
                    while Local::now().date_naive() == today {
                        sleep(Duration::from_secs(1));
                    }

                    log::info!("Midnight reached – loading tomorrow's timetable.");
                    continue 'day;
                }
            }
        },
    }
}
