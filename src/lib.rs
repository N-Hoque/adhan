pub mod model;

use std::{fs::File, io::BufReader, path::PathBuf};

pub use model::{AdhanCommands, AdhanListSubcommand};
use model::{AdhanParameters, Method};
use rodio::{cpal::traits::HostTrait, Decoder, Device, DeviceTrait, OutputStream, Sink};
use salah::{Coordinates, Local, Prayer, PrayerSchedule, PrayerTimes};

use crate::model::AdhanType;

pub static AUDIO_PATH: &str = "audio";
static SETTINGS_FILE: &str = "settings.yaml";

fn get_project_config_directory() -> Option<PathBuf> {
    directories_next::ProjectDirs::from("", "", "adhan").map(|project_dirs| project_dirs.config_dir().to_path_buf())
}

#[must_use]
pub fn read_config() -> AdhanParameters {
    let Some(config_dir) = get_project_config_directory() else {
        panic!("AGH")
    };

    let config_path = config_dir.join(SETTINGS_FILE);
    let file = File::open(config_path).expect("reading config file");

    serde_yaml::from_reader(file).expect("deserializing config file")
}

pub fn create_config(method: Method) {
    let Some(config_dir) = get_project_config_directory() else {
        panic!("AGH")
    };

    let config_path = config_dir.join(SETTINGS_FILE);
    let file = File::create(config_path).expect("opening config file");

    serde_yaml::to_writer(
        file,
        &AdhanParameters {
            coordinates: Coordinates::new(0.0, 0.0),
            parameters: method.parameters(),
        },
    )
    .expect("serializing config file");
}

pub fn play_adhan(prayer: Prayer, device: &str) {
    let Some(config_dir) = get_project_config_directory() else {
        panic!("CRITICAL ERROR: Program directory does not exist.")
    };

    let audio_config_path = config_dir.join(AUDIO_PATH);

    assert!(
        std::fs::metadata(&audio_config_path).is_ok(),
        "Audio folder is not present. Please create one at {}.",
        audio_config_path.display(),
    );

    // Get a output stream handle to the default physical sound device
    let Ok((_stream, stream_handle)) = get_device(device).map_or_else(OutputStream::try_default, |device| {
        OutputStream::try_from_device(&device)
    }) else {
        panic!("finding output device")
    };

    let adhan_type = if let Prayer::Fajr | Prayer::FajrTomorrow = prayer {
        AdhanType::Fajr
    } else {
        AdhanType::Normal
    };

    // Load a sound from a file, using a path relative to Cargo.toml
    let Ok(audio_file) = File::open(audio_config_path.join(format!("{}.mp3", adhan_type))) else {
        panic!(
            "Audio file not present for '{}'. Please put one in {} and name it '{}.mp3'",
            adhan_type,
            audio_config_path.display(),
            adhan_type
        )
    };

    let file = BufReader::new(audio_file);

    // Decode that sound file into a source
    let source = Decoder::new(file).unwrap();

    let sink = Sink::try_new(&stream_handle).unwrap();

    // Add a dummy source of the sake of the example.
    sink.append(source);

    // The sound plays in a separate thread. This call will block the current thread until the sink
    // has finished playing all its queued sounds.
    sink.sleep_until_end();
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
pub fn new_timetable(parameters: &AdhanParameters) -> PrayerTimes<Local> {
    PrayerSchedule::new()
        .with_date(&chrono::Local::now())
        .with_coordinates(parameters.coordinates())
        .with_parameters(parameters.parameters())
        .build()
        .map_or_else(
            |err| {
                eprintln!("Failed to calculate prayer times! - {err}");
                std::process::exit(1);
            },
            |ps| ps,
        )
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
