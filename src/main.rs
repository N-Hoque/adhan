use std::io::Write;

use adhan::{
    create_config, list_audio_devices, list_audio_hosts, new_timetable, play_adhan, read_config, AdhanCommands,
    AdhanListSubcommand, AUDIO_PATH,
};
use clap::Parser;
use salah::Prayer;

fn initialize_user_config_directory() {
    let Some(project_dirs) = directories_next::ProjectDirs::from("", "", "adhan") else {
        panic!("AGH")
    };

    let config_path = project_dirs.config_dir();
    let audio_path = &config_path.join(AUDIO_PATH);

    if std::fs::metadata(audio_path).is_err() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .create(config_path)
            .expect("creating config directory");

        println!("Adhan program initialized!");
        println!("To configure:");
        println!("- Generate a configuration file using 'adhan generate <METHOD>'");
        println!(
            "- Place Fajr adhan audio file at '{}/fajr.mp3' 'adhan generate <METHOD>'",
            audio_path.display()
        );
        println!(
            "- Place standard adhan audio file at '{}/normal.mp3' 'adhan generate <METHOD>'",
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
            play_adhan(if use_fajr { Prayer::Fajr } else { Prayer::Isha }, &audio_device);
        }
        AdhanCommands::Timetable => {
            let parameters = read_config();

            let timetable = new_timetable(&parameters);

            println!("{timetable}");
        }
        AdhanCommands::Run { audio_device } => {
            let parameters = read_config();

            let mut timetable = new_timetable(&parameters);

            loop {
                let current_time = chrono::Local::now();
                let (hours, minutes) = timetable.time_remaining(&current_time);
                if hours == 0 && minutes == 0 {
                    let current_prayer = timetable.next(&current_time);
                    print!("                                               \r");
                    print!("Prayer time is now!\r");
                    play_adhan(current_prayer, &audio_device);
                    timetable = new_timetable(&parameters);
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
