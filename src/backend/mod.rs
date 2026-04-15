pub mod rodio;

use ::rodio::Source;

#[cfg(target_os = "linux")]
pub mod pipewire;

#[cfg(target_os = "windows")]
pub mod wasapi;

use crate::model::AdhanError;

/// Trait that all platform audio backends must implement.
///
/// The interface is intentionally minimal: callers supply a lazily-decoded
/// `Source<Item = f32>` and the backend is responsible for streaming those
/// samples to the platform audio service and blocking until playback is
/// complete.
///
/// Rate and channel count are obtained from the `Source` itself via
/// `Iterator::sample_rate()` and `Source::channels()`, so the caller does
/// not need to pass them separately.
pub trait AudioBackend {
    /// Stream `source` to the platform audio output and block until done.
    ///
    /// The source must yield interleaved f32 PCM frames. The backend is free
    /// to pull samples from it on any thread, but must not call blocking I/O
    /// or allocate inside any real-time callback.
    fn play_blocking(&self, source: Box<dyn Source<Item = f32> + Send>) -> Result<(), AdhanError>;
}

// ── Platform alias ────────────────────────────────────────────────────────────
//
// Each target OS resolves `PlatformBackend` to its native implementation.
// For now every platform uses RodioBackend while the native backends are
// built out in subsequent phases.  Swapping a platform over is a one-line
// change here.

#[cfg(target_os = "linux")]
pub use self::pipewire::PipewireBackend as PlatformBackend;
#[cfg(target_os = "macos")]
pub use self::rodio::RodioBackend as PlatformBackend;
#[cfg(target_os = "windows")]
pub use self::wasapi::WasapiBackend as PlatformBackend;
