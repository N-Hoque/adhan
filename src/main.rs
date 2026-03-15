use std::{io::Write, thread::sleep, time::Duration};

use adhan::{
    create_config, initialize_user_config_directory, list_audio_devices, list_audio_hosts, new_timetable, play_adhan,
    read_config, AdhanCommands, AdhanListSubcommand,
};

use clap::Parser;
use salah::{Datelike, Event, Prayer};

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
        AdhanCommands::List(AdhanListSubcommand::Devices) => {
            list_audio_devices();
        }
        AdhanCommands::List(AdhanListSubcommand::Hosts) => {
            list_audio_hosts();
        }
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

                let current_time = chrono::Local::now();
                println!("{}", timetable.display(&current_time));
            }
        },
        AdhanCommands::Run { audio_device } => match read_config() {
            Err(err) => {
                log::error!("{}", err);
                std::process::exit(CONFIGURATION_READ_EXIT_CODE);
            }
            Ok(parameters) => {
                let mut timetable = new_timetable(&parameters);

                log::info!("Started Adhan!");

                loop {
                    let current_time = chrono::Local::now();
                    let expected_prayers = timetable.expected(&current_time);
                    let next_event = expected_prayers.next_event();
                    let next_time = *expected_prayers.next_time();
                    let event_name = if current_time.weekday() == chrono::Weekday::Fri {
                        next_event.friday_name()
                    } else {
                        next_event.name()
                    };

                    // How many seconds until the next event fires.
                    let secs_until_event = next_time.signed_duration_since(current_time).num_seconds();

                    if secs_until_event <= 0 {
                        // The event is due now — play and rebuild the timetable.
                        log::info!("{event_name} is now!");
                        if let Err(err) = play_adhan(next_event, &audio_device) {
                            log::error!("{}", err);
                            std::process::exit(PLAYBACK_EXIT_CODE);
                        }
                        timetable = new_timetable(&parameters);
                    } else if secs_until_event <= 60 {
                        // Within the final minute: poll every second for accuracy.
                        let hours = secs_until_event / 3600;
                        let minutes = (secs_until_event % 3600) / 60;
                        let seconds = secs_until_event % 60;
                        if matches!(next_event, Event::Prayer(_)) {
                            log::info!("{event_name} prayer starts in: {hours:>2}h {minutes:>2}m {seconds:>2}s");
                        } else {
                            log::info!("Waiting for: {hours:>2}h {minutes:>2}m {seconds:>2}s...");
                        }
                        let _ = std::io::stdout().flush();
                        sleep(Duration::from_secs(1));
                    } else {
                        // More than a minute away: sleep until 1 minute before
                        // the event, then the next iteration enters the fine-
                        // grained polling branch above.
                        let sleep_secs = (secs_until_event - 60).max(1) as u64;
                        let hours = secs_until_event / 3600;
                        let minutes = (secs_until_event % 3600) / 60;
                        if matches!(next_event, Event::Prayer(_)) {
                            log::info!("{event_name} prayer starts in: {hours:>2}h {minutes:>2}m – sleeping");
                        } else {
                            log::info!("Waiting for: {hours:>2}h {minutes:>2}m – sleeping");
                        }
                        let _ = std::io::stdout().flush();
                        sleep(Duration::from_secs(sleep_secs));
                    }
                }
            }
        },
    }
}
