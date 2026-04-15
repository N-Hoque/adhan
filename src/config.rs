use std::{
    fs::{DirBuilder, File},
    path::PathBuf,
};

use salah::Coordinates;

use crate::model::{AdhanError, AdhanParameters, Method};

static AUDIO_PATH: &str = "audio";
static SETTINGS_FILE: &str = "settings.yaml";

/// Returns the platform-appropriate base config directory for this application.
///
/// On Linux this is typically `~/.config/adhan/`. The path is resolved via
/// `directories_next` and is guaranteed to be absolute.
pub fn adhan_base_directory() -> Result<PathBuf, AdhanError> {
    directories_next::ProjectDirs::from("", "", "adhan")
        .ok_or_else(|| AdhanError::ConfigDir("cannot resolve configuration folder for 'adhan'".into()))
        .map(|dirs| dirs.config_dir().to_path_buf())
}

/// Returns the path to the audio subdirectory inside the base config directory.
pub fn adhan_audio_directory() -> Result<PathBuf, AdhanError> {
    adhan_base_directory().map(|p| p.join(AUDIO_PATH))
}

/// Creates the application's config directory tree on first run.
///
/// The expected layout is:
/// ```text
/// <config>/
///   audio/
///     fajr/
///     normal/
/// ```
///
/// This is a no-op if the base directory already exists.
pub fn initialize_user_config_directory() -> Result<(), AdhanError> {
    if adhan_base_directory().is_ok_and(|dir| !dir.exists()) {
        let audio_path = adhan_audio_directory()?;

        for subdir in [
            audio_path.as_path(),
            &audio_path.join("fajr"),
            &audio_path.join("normal"),
        ] {
            DirBuilder::new()
                .recursive(true)
                .create(subdir)
                .map_err(AdhanError::Io)?;
        }

        log::info!("Adhan program initialized!");
        log::info!("To configure:");
        log::info!("  1. Run 'adhan generate <METHOD>' to create a config file.");
        log::info!("  2. Edit the generated file to set your coordinates.");
        log::info!("  3. Place Fajr audio files in '{}/fajr'", audio_path.display());
        log::info!("  4. Place standard audio files in '{}/normal'", audio_path.display());
    }

    Ok(())
}

/// Reads and deserialises `settings.yaml` from the config directory.
pub fn read_config() -> Result<AdhanParameters, AdhanError> {
    let config_path = adhan_base_directory()?.join(SETTINGS_FILE);
    let file = File::open(config_path).map_err(AdhanError::Io)?;
    serde_yaml::from_reader(file).map_err(AdhanError::ConfigParse)
}

/// Serialises a default `AdhanParameters` (zero coordinates, given method)
/// and writes it to `settings.yaml` in the config directory.
///
/// The user should edit the coordinates after generation.
pub fn create_config(method: Method) -> Result<(), AdhanError> {
    let config_path = adhan_base_directory()?.join(SETTINGS_FILE);
    let file = File::create(config_path).map_err(AdhanError::Io)?;

    serde_yaml::to_writer(
        file,
        &AdhanParameters {
            coordinates: Coordinates::new(0.0, 0.0),
            parameters: method.parameters(),
        },
    )
    .map_err(AdhanError::ConfigParse)
}
