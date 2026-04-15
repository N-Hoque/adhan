use std::{io::BufReader, path::PathBuf};

/// Audio file extensions that rodio can decode with its default feature set.
///
/// rodio's default features enable: `mp3` (via symphonia), `wav` (via hound),
/// `flac` (via claxon), and `vorbis` / OGG (via lewton). Any file with one of
/// these extensions will be accepted by `select_audio_file` and handed to
/// `rodio::Decoder` for decoding.
///
/// If you enable additional rodio features (e.g. `symphonia-aac`) add the
/// corresponding extension here.
const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "ogg"];

use rand::seq::IndexedRandom;
use rodio::Decoder;
use salah::{Event, Prayer};

use crate::{
    backend::{AudioBackend, PlatformBackend},
    config::adhan_audio_directory,
    model::{AdhanError, AdhanType},
};

/// Chooses a random `.mp3` file from the audio subfolder that corresponds to
/// `adhan_type`.
///
/// Returns the absolute path to the chosen file, or an error if the directory
/// is missing or contains no playable files.
///
/// Extracting this step from `play_adhan` keeps the selection logic independently
/// testable and makes it straightforward to swap in alternative selection
/// strategies (e.g. sequential, weighted) later.
pub(crate) fn select_audio_file(adhan_type: AdhanType) -> Result<PathBuf, AdhanError> {
    let audio_dir = adhan_audio_directory()?;

    if std::fs::metadata(&audio_dir).is_err() {
        return Err(AdhanError::AudioDirMissing { path: audio_dir });
    }

    let subfolder = audio_dir.join(adhan_type.subfolder_name());

    let candidates: Vec<PathBuf> = std::fs::read_dir(&subfolder)
        .map_err(AdhanError::Io)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().is_ok_and(|t| t.is_file())
                && entry.path().extension().and_then(|e| e.to_str()).is_some_and(|ext| {
                    SUPPORTED_EXTENSIONS
                        .iter()
                        .any(|&supported| ext.eq_ignore_ascii_case(supported))
                })
        })
        .map(|entry| entry.path())
        .collect();

    let mut rng = rand::rng();

    candidates
        .choose(&mut rng)
        .cloned()
        .ok_or_else(|| AdhanError::NoAudioFiles { path: subfolder })
}

/// Plays the appropriate adhan audio for the given prayer event.
///
/// The call blocks until playback is complete. Events that do not correspond
/// to a playable prayer (`Qiyam`, `Sunrise`, `Restricted`) are silently
/// ignored and return `Ok(())` immediately.
///
/// # Playback pipeline
///
/// 1. Determine the `AdhanType` from the event.
/// 2. Select a random audio file via [`select_audio_file`].
/// 3. Open and decode the file into an f32 PCM source.
/// 4. Hand the source to the compile-time [`PlatformBackend`] to stream to the
///    platform audio service.
pub fn play_adhan(prayer: Event) -> Result<(), AdhanError> {
    let adhan_type = match prayer {
        Event::Qiyam | Event::Sunrise | Event::Restricted(_) => return Ok(()),
        Event::Prayer(Prayer::Fajr) => AdhanType::Fajr,
        _ => AdhanType::Normal,
    };

    let audio_file_path = select_audio_file(adhan_type)?;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(&audio_file_path)
        .map_err(AdhanError::Io)?;

    let decoder = Decoder::new(BufReader::new(file)).map_err(AdhanError::AudioDecode)?;
    let source = Box::new(rodio::source::Source::convert_samples::<f32>(decoder));

    PlatformBackend.play_blocking(source)
}

/// A `PrayerEventHandler` that plays the appropriate adhan audio file.
///
/// This is a thin wrapper around [`play_adhan`] that plugs audio playback
/// into the scheduler's handler system. Decoupling it from the run loop
/// means audio is one of potentially many independent actions taken when a
/// prayer fires.
pub struct AdhanPlayer;

impl crate::schedule::PrayerEventHandler for AdhanPlayer {
    fn on_prayer(&self, event: &salah::Event, event_name: &str) -> Result<(), crate::model::AdhanError> {
        log::info!("{} – playing adhan", event_name);
        play_adhan(*event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AdhanType::Fajr` must map to the `"fajr"` subfolder name, and
    /// `AdhanType::Normal` must map to `"normal"`.  These strings are the
    /// on-disk directory names, so a regression here would silently break
    /// audio file lookup.
    #[test]
    fn adhan_type_subfolder_names_are_correct() {
        assert_eq!(AdhanType::Fajr.subfolder_name(), "fajr");
        assert_eq!(AdhanType::Normal.subfolder_name(), "normal");
    }

    /// Every entry in SUPPORTED_EXTENSIONS must be lowercase so that the
    /// case-insensitive comparison in select_audio_file works correctly.
    /// An uppercase entry like "MP3" would never match because we call
    /// eq_ignore_ascii_case on the *file's* extension against our constant —
    /// if the constant itself is uppercase the comparison still works, but
    /// this test documents the expected convention.
    #[test]
    fn supported_extensions_are_all_lowercase() {
        for ext in SUPPORTED_EXTENSIONS {
            assert_eq!(*ext, ext.to_ascii_lowercase(), "extension '{ext}' should be lowercase");
        }
    }

    /// Non-prayer events must be silently skipped — `play_adhan` should
    /// return `Ok(())` without touching the filesystem or audio backend.
    /// We verify this by calling with events that have no audio path and
    /// confirming there is no error (no audio dir is needed on disk).
    #[test]
    fn play_adhan_skips_non_prayer_events() {
        // These should return Ok(()) immediately without any filesystem access.
        assert!(play_adhan(Event::Qiyam).is_ok());
        assert!(play_adhan(Event::Sunrise).is_ok());
    }
}
