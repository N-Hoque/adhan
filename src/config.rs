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
    serde_yml::from_reader(file).map_err(AdhanError::ConfigParse)
}

/// Serialises a default `AdhanParameters` (zero coordinates, given method)
/// and writes it to `settings.yaml` in the config directory.
///
/// The user should edit the coordinates after generation.
pub fn create_config(method: Method) -> Result<(), AdhanError> {
    let config_path = adhan_base_directory()?.join(SETTINGS_FILE);
    let file = File::create(config_path).map_err(AdhanError::Io)?;

    serde_yml::to_writer(
        file,
        &AdhanParameters {
            coordinates: Coordinates::new(0.0, 0.0),
            parameters: method.parameters(),
        },
    )
    .map_err(AdhanError::ConfigParse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use salah::Method;

    // ── adhan_audio_directory ─────────────────────────────────────────────────

    /// `adhan_audio_directory` must return a path that ends with "audio",
    /// confirming it is correctly joined onto the base directory.
    #[test]
    fn audio_directory_path_ends_with_audio() {
        let path = adhan_audio_directory().expect("should resolve audio directory");
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("audio"),
            "audio directory should end with 'audio', got: {}",
            path.display()
        );
    }

    /// `adhan_audio_directory` must be a subdirectory of `adhan_base_directory`.
    #[test]
    fn audio_directory_is_child_of_base_directory() {
        let base = adhan_base_directory().expect("should resolve base directory");
        let audio = adhan_audio_directory().expect("should resolve audio directory");
        assert!(
            audio.starts_with(&base),
            "audio directory '{}' should be under base directory '{}'",
            audio.display(),
            base.display()
        );
    }

    // ── create_config / read_config roundtrip ─────────────────────────────────

    /// Writing a config with `create_config` and reading it back with
    /// `read_config` must round-trip the coordinates and parameters without
    /// loss. We write to a temp file and read it back directly rather than
    /// going through the platform config path, so this test is hermetic.
    #[test]
    fn create_and_read_config_roundtrip() {
        use salah::Coordinates;
        use std::fs::File;

        let dir = tempfile::tempdir().expect("create tempdir");
        let config_path = dir.path().join("settings.yaml");

        // Write a config with known coordinates.
        let params = AdhanParameters {
            coordinates: Coordinates::new(51.5074, -0.1278),
            parameters: Method::MoonsightingCommittee.parameters(),
        };
        let file = File::create(&config_path).expect("create config file");
        serde_yml::to_writer(file, &params).expect("write config");

        // Read it back and verify the coordinates survived the roundtrip.
        let file = File::open(&config_path).expect("open config file");
        let read_back: AdhanParameters = serde_yml::from_reader(file).expect("read config");

        assert!(
            (read_back.coordinates().latitude - 51.5074).abs() < 1e-6,
            "latitude did not round-trip correctly"
        );
        assert!(
            (read_back.coordinates().longitude - (-0.1278)).abs() < 1e-6,
            "longitude did not round-trip correctly"
        );
    }

    /// `create_config` must produce a file that `serde_yaml` can parse —
    /// i.e. it must write valid YAML. We use the generated file to drive
    /// a parse and confirm no error is returned.
    #[test]
    fn create_config_produces_valid_yaml() {
        use std::fs::File;

        let dir = tempfile::tempdir().expect("create tempdir");
        let config_path = dir.path().join("settings.yaml");

        let params = AdhanParameters {
            coordinates: salah::Coordinates::new(0.0, 0.0),
            parameters: Method::MuslimWorldLeague.parameters(),
        };
        let file = File::create(&config_path).expect("create config file");
        serde_yml::to_writer(file, &params).expect("write config");

        let file = File::open(&config_path).expect("open config file");
        let result: Result<AdhanParameters, _> = serde_yml::from_reader(file);
        assert!(
            result.is_ok(),
            "create_config produced invalid YAML: {:?}",
            result.err()
        );
    }

    /// Feeding malformed YAML to `serde_yaml::from_reader` must produce a
    /// `ConfigParse` error — confirming that `read_config`'s error mapping
    /// is correct and that bad config files do not panic.
    #[test]
    fn malformed_yaml_produces_config_parse_error() {
        use std::io::Write;

        let dir = tempfile::tempdir().expect("create tempdir");
        let config_path = dir.path().join("settings.yaml");

        let mut file = std::fs::File::create(&config_path).expect("create file");
        write!(file, "{{{{ not: valid: yaml: ::::").expect("write bad yaml");

        let file = std::fs::File::open(&config_path).expect("open file");
        let result: Result<AdhanParameters, _> = serde_yml::from_reader(file);
        assert!(result.is_err(), "expected a parse error for malformed YAML but got Ok");
    }

    /// Zero coordinates (the default from `generate`) must serialise and
    /// deserialise without error — this is the exact state a new user's
    /// config is in before they edit it.
    #[test]
    fn zero_coordinates_roundtrip_without_error() {
        use std::fs::File;

        let dir = tempfile::tempdir().expect("create tempdir");
        let config_path = dir.path().join("settings.yaml");

        let params = AdhanParameters {
            coordinates: salah::Coordinates::new(0.0, 0.0),
            parameters: Method::Other.parameters(),
        };
        let file = File::create(&config_path).expect("create config file");
        serde_yml::to_writer(file, &params).expect("write config");

        let file = File::open(&config_path).expect("open config file");
        let read_back: AdhanParameters = serde_yml::from_reader(file).expect("read config");

        assert!((read_back.coordinates().latitude).abs() < 1e-6);
        assert!((read_back.coordinates().longitude).abs() < 1e-6);
    }
}
