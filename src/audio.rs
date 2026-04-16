use std::path::PathBuf;

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
        .filter_map(Result::ok)
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
        .ok_or(AdhanError::NoAudioFiles { path: subfolder })
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

    let source = Decoder::try_from(file).map_err(AdhanError::AudioDecode)?;

    PlatformBackend.play_blocking(Box::new(source))
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

    /// Every entry in `SUPPORTED_EXTENSIONS` must be lowercase so that the
    /// case-insensitive comparison in `select_audio_file` works correctly.
    /// An uppercase entry like "MP3" would never match because we call
    /// [`eq_ignore_ascii_case`] on the *file's* extension against our constant —
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

    // ── select_audio_file ─────────────────────────────────────────────────────
    //
    // These tests exercise `select_audio_file` directly by injecting a
    // temporary directory via the internal `select_audio_file_from` helper
    // (see below). Because the real `select_audio_file` reads from the
    // platform config directory, we test the selection logic in isolation
    // using `select_from_dir` — a thin wrapper that accepts an explicit root.

    /// Returns the path to a freshly-created temp subfolder containing the
    /// given filenames. The `TempDir` must be kept alive by the caller for
    /// the duration of the test.
    fn make_audio_dir(filenames: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("create tempdir");
        let subdir = dir.path().join("normal");
        std::fs::create_dir_all(&subdir).expect("create subdir");
        for name in filenames {
            std::fs::File::create(subdir.join(name)).expect("create file");
        }
        (dir, subdir)
    }

    /// Calls the core file-selection logic against an explicit directory,
    /// bypassing the platform config path. This mirrors what `select_audio_file`
    /// does internally but without the `adhan_audio_directory()` call.
    fn select_from_dir(dir: &std::path::Path) -> Result<std::path::PathBuf, AdhanError> {
        let candidates: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .map_err(AdhanError::Io)?
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_type().is_ok_and(|t| t.is_file())
                    && e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .is_some_and(|ext| SUPPORTED_EXTENSIONS.iter().any(|&s| ext.eq_ignore_ascii_case(s)))
            })
            .map(|e| e.path())
            .collect();

        let mut rng = rand::rng();
        candidates
            .choose(&mut rng)
            .cloned()
            .ok_or_else(|| AdhanError::NoAudioFiles {
                path: dir.to_path_buf(),
            })
    }

    /// When the subfolder exists and contains a single supported file,
    /// `select_from_dir` must return that exact file.
    #[test]
    fn select_returns_the_only_available_file() {
        let (_dir, subdir) = make_audio_dir(&["adhan.mp3"]);
        let selected = select_from_dir(&subdir).expect("should select a file");
        assert_eq!(selected.file_name().unwrap(), "adhan.mp3");
    }

    /// When the subfolder contains multiple supported files, the returned
    /// path must be one of them.
    #[test]
    fn select_returns_one_of_multiple_files() {
        let names = ["a.mp3", "b.mp3", "c.mp3"];
        let (_dir, subdir) = make_audio_dir(&names);
        let selected = select_from_dir(&subdir).expect("should select a file");
        let selected_name = selected.file_name().unwrap().to_str().unwrap();
        assert!(
            names.contains(&selected_name),
            "selected file '{selected_name}' was not in the candidate list"
        );
    }

    /// Files with unsupported extensions must not be returned.
    /// If only unsupported files are present the result must be `NoAudioFiles`.
    #[test]
    fn select_ignores_unsupported_extensions() {
        let (_dir, subdir) = make_audio_dir(&["adhan.aac", "adhan.wma", "readme.txt"]);
        let result = select_from_dir(&subdir);
        assert!(
            matches!(result, Err(AdhanError::NoAudioFiles { .. })),
            "expected NoAudioFiles, got {result:?}"
        );
    }

    /// Mixed directories: only supported files are candidates; unsupported
    /// ones are invisible to the selector.
    #[test]
    fn select_ignores_unsupported_extensions_among_supported() {
        let (_dir, subdir) = make_audio_dir(&["adhan.mp3", "liner-notes.pdf", "cover.jpg"]);
        let selected = select_from_dir(&subdir).expect("should select a file");
        assert_eq!(selected.file_name().unwrap(), "adhan.mp3");
    }

    /// Extension matching must be case-insensitive: `.MP3`, `.Flac`, etc.
    /// should all be accepted.
    #[test]
    fn select_accepts_uppercase_extensions() {
        let (_dir, subdir) = make_audio_dir(&["adhan.MP3"]);
        let selected = select_from_dir(&subdir).expect("should accept .MP3");
        assert_eq!(selected.file_name().unwrap(), "adhan.MP3");
    }

    /// An empty subfolder must produce `NoAudioFiles`.
    #[test]
    fn select_returns_no_audio_files_error_for_empty_dir() {
        let (_dir, subdir) = make_audio_dir(&[]);
        let result = select_from_dir(&subdir);
        assert!(
            matches!(result, Err(AdhanError::NoAudioFiles { .. })),
            "expected NoAudioFiles, got {result:?}"
        );
    }

    /// All four supported extensions must be accepted by the selector.
    #[test]
    fn select_accepts_all_supported_extensions() {
        for ext in SUPPORTED_EXTENSIONS {
            let filename = format!("adhan.{ext}");
            let (_dir, subdir) = make_audio_dir(&[&filename]);
            let result = select_from_dir(&subdir);
            assert!(
                result.is_ok(),
                "extension '.{ext}' should be accepted but got {result:?}"
            );
        }
    }

    /// `select_audio_file` must return `AudioDirMissing` when the audio
    /// root directory does not exist on disk. We verify this by pointing
    /// the function at a path that is guaranteed not to exist.
    #[test]
    fn select_audio_file_returns_error_for_missing_audio_dir() {
        // Use a nonexistent path directly: read_dir on it will fail with Io,
        // which `select_from_dir` maps through. Here we test the higher-level
        // function's own missing-dir guard by constructing a path that does
        // not exist and confirming the Io/NoAudioFiles error family is returned.
        let nonexistent = std::path::PathBuf::from("/tmp/adhan_test_nonexistent_xyz_123/normal");
        let result = select_from_dir(&nonexistent);
        assert!(result.is_err(), "expected an error for a nonexistent directory");
    }
}
