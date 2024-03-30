use std::io::Write;

use adhan::{
    create_config, initialize_user_config_directory, list_audio_devices, list_audio_hosts, model::AdhanError,
    new_timetable, play_adhan, read_config, AdhanCommands, AdhanListSubcommand,
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

fn main() -> Result<(), AdhanError> {
    initialise_logging();

    initialize_user_config_directory()?;

    match AdhanCommands::parse() {
        AdhanCommands::List(AdhanListSubcommand::Devices) => {
            list_audio_devices();
            Ok(())
        }
        AdhanCommands::List(AdhanListSubcommand::Hosts) => {
            list_audio_hosts();
            Ok(())
        }
        AdhanCommands::Generate { method } => create_config(method),
        AdhanCommands::Test { audio_device, use_fajr } => play_adhan(
            if use_fajr {
                Event::Prayer(Prayer::Fajr)
            } else {
                Event::Prayer(Prayer::Isha)
            },
            &audio_device,
        ),
        AdhanCommands::Timetable => {
            let parameters = read_config()?;

            let timetable = new_timetable(&parameters);

            let current_time = chrono::Local::now();
            println!("{}", timetable.display(&current_time));
            Ok(())
        }
        AdhanCommands::Run { audio_device } => {
            let parameters = read_config()?;

            let mut timetable = new_timetable(&parameters);

            let mut log_event = false;

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
                    play_adhan(next_event, &audio_device)?;
                    timetable = new_timetable(&parameters);
                } else if matches!(next_event, Event::Prayer(_)) {
                    log::info!("{event_name} prayer starts in:{hours:>2}h {minutes:>2}m");
                } else {
                    log::info!("Waiting for:{hours:>2}h {minutes:>2}...");
                }
                let _ = std::io::stdout().flush();
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}
