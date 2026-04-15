use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use salah::{Coordinates, Parameters};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdhanError {
    /// A filesystem operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The config file could not be parsed.
    #[error("config parse error: {0}")]
    ConfigParse(#[from] serde_yaml::Error),

    /// The platform config directory could not be resolved.
    #[error("config directory unavailable: {0}")]
    ConfigDir(String),

    /// The audio directory was expected to exist but does not.
    #[error("audio directory not found at {path}")]
    AudioDirMissing { path: PathBuf },

    /// The audio directory exists but contains no playable .mp3 files.
    #[error("no audio files found in {path}")]
    NoAudioFiles { path: PathBuf },

    /// The chosen audio file could not be decoded.
    #[error("audio decode error: {0}")]
    AudioDecode(#[from] rodio::decoder::DecoderError),

    /// The platform audio backend failed to open or play the stream.
    #[error("audio playback error: {0}")]
    AudioPlayback(String),

    /// The desktop notification could not be delivered.
    #[error("notification error: {0}")]
    Notification(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdhanType {
    Normal,
    Fajr,
}

impl AdhanType {
    /// Returns the name of the audio subfolder that corresponds to this adhan
    /// type.
    ///
    /// These strings are the literal directory names on disk (`audio/fajr/`,
    /// `audio/normal/`). Having an explicit method — rather than relying on a
    /// `Display` impl — makes the coupling between this type and the filesystem
    /// layout visible and keeps it in one place.
    pub(crate) const fn subfolder_name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Fajr => "fajr",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AdhanParameters {
    pub coordinates: Coordinates,
    pub parameters: Parameters,
}

impl AdhanParameters {
    pub(crate) fn coordinates(&self) -> Coordinates {
        self.coordinates.clone()
    }

    pub(crate) fn parameters(&self) -> Parameters {
        self.parameters.clone()
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Method {
    MuslimWorldLeague,
    Egyptian,
    Karachi,
    UmmAlQura,
    Dubai,
    MoonsightingCommittee,
    NorthAmerica,
    Kuwait,
    Qatar,
    Singapore,
    Tehran,
    Turkey,
    Other,
}

impl Method {
    pub(crate) fn parameters(self) -> Parameters {
        match self {
            Self::MuslimWorldLeague => salah::Method::MuslimWorldLeague.parameters(),
            Self::Egyptian => salah::Method::Egyptian.parameters(),
            Self::Karachi => salah::Method::Karachi.parameters(),
            Self::UmmAlQura => salah::Method::UmmAlQura.parameters(),
            Self::Dubai => salah::Method::Dubai.parameters(),
            Self::MoonsightingCommittee => salah::Method::MoonsightingCommittee.parameters(),
            Self::NorthAmerica => salah::Method::NorthAmerica.parameters(),
            Self::Kuwait => salah::Method::Kuwait.parameters(),
            Self::Qatar => salah::Method::Qatar.parameters(),
            Self::Singapore => salah::Method::Singapore.parameters(),
            Self::Tehran => salah::Method::Tehran.parameters(),
            Self::Turkey => salah::Method::Turkey.parameters(),
            Self::Other => salah::Method::Other.parameters(),
        }
    }
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about=None)]
pub enum AdhanCommands {
    /// Run adhan player
    Run,
    /// Test audio playback
    Test {
        /// Play Fajr Adhan
        #[clap(short = 'f', long, required = false)]
        use_fajr: bool,
    },
    /// Show prayer timetable
    Timetable,
    /// Generate a config file from a given method
    Generate {
        /// The name of the calculation method to generate a sample config from
        method: Method,
    },
}
