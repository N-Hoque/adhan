use std::io::Write;

use adhan::{
    create_config, initialize_user_config_directory, list_audio_devices, list_audio_hosts, new_timetable, play_adhan,
    read_config, AdhanCommands, AdhanListSubcommand,
};
use clap::Parser;
use salah::{Datelike, Event, Prayer};

fn initialise_logging() {
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    )
    .expect("initialising logger");
}

const CONFIGURATION_INIT_EXIT_CODE: i32 = 1;
const CONFIGURATION_CREATE_EXIT_CODE: i32 = 2;
const CONFIGURATION_READ_EXIT_CODE: i32 = 3;
const PLAYBACK_EXIT_CODE: i32 = 4;

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
                    let (hours, minutes) = timetable.time_remaining(&current_time);
                    let expected_prayers = timetable.expected(&current_time);
                    let next_event = expected_prayers.next_event();
                    let event_name = if current_time.weekday() == chrono::Weekday::Fri {
                        next_event.friday_name()
                    } else {
                        next_event.name()
                    };

                    if hours == 0 && minutes == 0 {
                        log::info!("{event_name} is now!");
                        if let Err(err) = play_adhan(next_event, &audio_device) {
                            log::error!("{}", err);
                            std::process::exit(PLAYBACK_EXIT_CODE);
                        }
                        timetable = new_timetable(&parameters);
                    } else if (hours > 0 && minutes == 0) || (hours == 0 && minutes % 5 == 0) {
                        if matches!(next_event, Event::Prayer(_)) {
                            log::info!("{event_name} prayer starts in:{hours:>2}h {minutes:>2}m");
                        } else {
                            log::info!("Waiting for:{hours:>2}h {minutes:>2}...");
                        }
                    }
                    let _ = std::io::stdout().flush();
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        },
    }
}
