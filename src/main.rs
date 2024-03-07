use std::io::Write;

use adhan::{
    create_config, list_audio_devices, list_audio_hosts, new_timetable, play_adhan, read_config, AdhanCommands,
    AdhanListSubcommand, AdhanType, AUDIO_PATH,
};
use clap::Parser;
use salah::Prayer;

fn initialize_user_config_directory() {
    let Some(project_dirs) = directories_next::ProjectDirs::from("", "", "adhan") else {
        panic!("AGH")
    };

    let config_path = project_dirs.config_dir();

    if std::fs::metadata(config_path.join(AUDIO_PATH)).is_err() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .create(config_path)
            .expect("creating config directory");
    }
}

fn main() {
    initialize_user_config_directory();

    let args = AdhanCommands::parse();

    match args {
        AdhanCommands::List(AdhanListSubcommand::Devices) => {
            list_audio_devices();
        }
        AdhanCommands::List(AdhanListSubcommand::Hosts) => {
            list_audio_hosts();
        }
        AdhanCommands::Generate { method } => {
            create_config(method);
        }
        AdhanCommands::Test { audio_device } => {
            play_adhan(AdhanType::Normal, &audio_device);
        }
        AdhanCommands::Timetable => {
            let parameters = read_config();

            let timetable = new_timetable(&parameters);

            println!("{timetable}");
        }
        AdhanCommands::Run { audio_device } => {
            let parameters = read_config();

            let mut timetable = new_timetable(&parameters);

            let mut next_prayer = timetable.next(&chrono::Local::now());

            loop {
                let current_time = chrono::Local::now();
                let (hours, minutes) = timetable.time_remaining(&current_time);
                if hours == 0 && minutes == 0 {
                    print!("                                               \r");
                    print!("Prayer time is now!\r");
                    if matches!(
                        next_prayer,
                        Prayer::Fajr
                            | Prayer::FajrTomorrow
                            | Prayer::Dhuhr
                            | Prayer::Asr
                            | Prayer::Maghrib
                            | Prayer::Isha
                    ) {
                        play_adhan(next_prayer, &audio_device)
                    }
                    timetable = new_timetable(&parameters);
                    next_prayer = timetable.next(&current_time);
                    print!("                                               \r");
                } else {
                    print!("Next prayer starts in {hours:>2}h {minutes:>2}m\r");
                }
                let _ = std::io::stdout().flush();
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}
