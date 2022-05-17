use clap::{Parser, Subcommand, ValueEnum};
use salah::{Coordinates, Parameters};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AdhanParameters {
    pub coordinates: Coordinates,
    pub parameters: Parameters,
}

impl AdhanParameters {
    pub(crate) const fn coordinates(&self) -> Coordinates {
        self.coordinates
    }

    pub(crate) const fn parameters(&self) -> Parameters {
        self.parameters
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
        audio_device: String,
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
