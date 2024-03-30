pub mod model;

use std::{
    fs::{DirBuilder, File},
    io::BufReader,
    path::PathBuf,
};

pub use model::{AdhanCommands, AdhanListSubcommand};
use model::{AdhanError, AdhanParameters, Method};
use rodio::{cpal::traits::HostTrait, Decoder, Device, DeviceTrait, OutputStream, Sink};
use salah::{Coordinates, Event, Local, Prayer, Schedule, Times};

use crate::model::{AdhanAudioError, AdhanType};

static AUDIO_PATH: &str = "audio";
static SETTINGS_FILE: &str = "settings.yaml";

pub fn initialize_user_config_directory() -> Result<(), AdhanError> {
    if adhan_base_directory().is_ok_and(|dir| !dir.exists()) {
        let audio_path = &adhan_audio_directory()?;

        DirBuilder::new()
            .recursive(true)
            .create(audio_path)
            .map_err(AdhanError::IO)?;

        log::info!("Adhan program initialized!");
        log::info!("To configure:");
        log::info!("- Generate a configuration file using 'adhan generate <METHOD>'");
        log::info!("- Place Fajr adhan audio file at '{}/fajr.mp3'", audio_path.display());
        log::info!(
            "- Place standard adhan audio file at '{}/normal.mp3'",
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

    serde_yaml::from_reader(file).map_err(AdhanError::Serialisation)
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
    .map_err(AdhanError::Serialisation)
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
        .map_err(|err| AdhanError::Audio(AdhanAudioError::Stream(err)))?;

    // Load a sound from a file, using a path relative to Cargo.toml
    let audio_file = File::open(audio_config_path.join(format!("{adhan_type}.mp3"))).map_err(AdhanError::IO)?;

    let file = BufReader::new(audio_file);

    // Decode that sound file into a source
    let source = Decoder::new(file).map_err(|err| AdhanError::Audio(AdhanAudioError::Decode(err)))?;

    let sink = Sink::try_new(&stream_handle).map_err(|err| AdhanError::Audio(AdhanAudioError::Playback(err)))?;

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
            log::info!("{idx}: {device}");
        }
    }
}

pub fn list_audio_hosts() {
    for (idx, device) in rodio::cpal::available_hosts().iter().enumerate() {
        log::info!("{}: {}", idx, device.name());
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
