use std::{fs::DirBuilder, io::Write};

use adhan::{
    adhan_audio_directory, adhan_base_directory, create_config, list_audio_devices, list_audio_hosts, new_timetable,
    play_adhan, read_config, AdhanCommands, AdhanListSubcommand,
};
use clap::Parser;
use salah::{Datelike, Event, Prayer};

fn initialize_user_config_directory() {
    if adhan_base_directory().is_some_and(|dir| !dir.exists()) {
        let Some(ref audio_path) = adhan_audio_directory() else {
            panic!("get audio config directory")
        };

        DirBuilder::new()
            .recursive(true)
            .create(audio_path)
            .expect("creating config directory");

        println!("Adhan program initialized!");
        println!("To configure:");
        println!("- Generate a configuration file using 'adhan generate <METHOD>'");
        println!("- Place Fajr adhan audio file at '{}/fajr.mp3'", audio_path.display());
        println!(
            "- Place standard adhan audio file at '{}/normal.mp3'",
            audio_path.display()
        );
    }
}

fn main() {
    initialize_user_config_directory();

    match AdhanCommands::parse() {
        AdhanCommands::List(AdhanListSubcommand::Devices) => {
            list_audio_devices();
        }
        AdhanCommands::List(AdhanListSubcommand::Hosts) => {
            list_audio_hosts();
        }
        AdhanCommands::Generate { method } => {
            create_config(method);
        }
        AdhanCommands::Test { audio_device, use_fajr } => {
            play_adhan(
                if use_fajr {
                    Event::Prayer(Prayer::Fajr)
                } else {
                    Event::Prayer(Prayer::Isha)
                },
                &audio_device,
            );
        }
        AdhanCommands::Timetable => {
            let parameters = read_config();

            let timetable = new_timetable(&parameters);

            let current_time = chrono::Local::now();
            println!("{}", timetable.display(&current_time));
        }
        AdhanCommands::Run { audio_device } => {
            let parameters = read_config();

            let mut timetable = new_timetable(&parameters);

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
                    print!("                                                        \r");
                    print!("{event_name} is now!\r");
                    play_adhan(next_event, &audio_device);
                    timetable = new_timetable(&parameters);
                    print!("                                                        \r");
                } else if matches!(next_event, Event::Prayer(_)) {
                    print!("{event_name} prayer starts in:{hours:>2}h {minutes:>2}m\r");
                } else {
                    print!("Waiting for:{hours:>2}h {minutes:>2}...\r");
                }
                let _ = std::io::stdout().flush();
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}
