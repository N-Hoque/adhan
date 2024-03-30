use clap::{Parser, Subcommand, ValueEnum};
use salah::{Coordinates, Parameters};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdhanAudioError {
    #[error("stream failure {0}")]
    Stream(#[from] rodio::StreamError),
    #[error("decode failure {0}")]
    Decode(#[from] rodio::decoder::DecoderError),
    #[error("playback failure {0}")]
    Playback(#[from] rodio::PlayError),
}

#[derive(Debug, Error)]
pub enum AdhanError {
    #[error("file IO failure")]
    IO(#[from] std::io::Error),
    #[error("file IO failure")]
    Serialisation(#[from] serde_yaml::Error),
    #[error("configuration failure {0}")]
    Configuration(String),
    #[error("audio handler failed: {0}")]
    Audio(AdhanAudioError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdhanType {
    Normal,
    Fajr,
}

impl std::fmt::Display for AdhanType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "normal"),
            Self::Fajr => write!(f, "fajr"),
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
    Run {
        #[clap(default_value = "default")]
        /// The output audio device to play the Adhan from
        audio_device: String,
    },
    /// Test audio playback
    Test {
        /// The output audio device to play the Adhan from
        #[clap(default_value = "default")]
        audio_device: String,
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
    #[command(subcommand)]
    /// List audio components
    List(AdhanListSubcommand),
}

#[derive(Debug, Subcommand)]
pub enum AdhanListSubcommand {
    /// List audio devices
    Devices,
    /// List audio hosts
    Hosts,
}
