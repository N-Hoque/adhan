pub mod rodio;

use crate::model::AdhanError;

/// Trait that all platform audio backends must implement.
///
/// The interface is intentionally minimal: callers pre-decode audio to
/// interleaved f32 PCM and hand it off here. The backend is responsible
/// for getting those samples to the platform audio service and blocking
/// until playback is complete.
pub trait AudioBackend {
    /// Play `samples` (interleaved f32 PCM) to completion, then return.
    ///
    /// - `samples`  – interleaved PCM frames, e.g. [L0, R0, L1, R1, …]
    /// - `rate`     – sample rate in Hz (e.g. 44100, 48000)
    /// - `channels` – number of channels (1 = mono, 2 = stereo)
    fn play_blocking(&self, samples: &[f32], rate: u32, channels: u16) -> Result<(), AdhanError>;
}

// ── Platform alias ────────────────────────────────────────────────────────────
//
// Each target OS resolves `PlatformBackend` to its native implementation.
// For now every platform uses RodioBackend while the native backends are
// built out in subsequent phases.  Swapping a platform over is a one-line
// change here.

#[cfg(target_os = "linux")]
pub use self::rodio::RodioBackend as PlatformBackend;

#[cfg(target_os = "macos")]
pub use self::rodio::RodioBackend as PlatformBackend;

#[cfg(target_os = "windows")]
pub use self::rodio::RodioBackend as PlatformBackend;
