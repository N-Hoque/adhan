use rodio::{OutputStream, Sink};

use crate::{backend::AudioBackend, model::AdhanError};

/// Audio backend that uses `rodio` for playback.
///
/// This is the transitional backend used on all platforms while the native
/// per-OS backends are built out. It wraps rodio's `OutputStream` +
/// `Sink` pair and blocks until the sink drains, matching the semantics
/// expected by `play_adhan`.
///
/// Unlike the previous `play_adhan` implementation, this backend receives
/// pre-decoded f32 PCM samples rather than a file path — decoding is the
/// caller's responsibility and happens in `play_adhan` before this is
/// invoked.
pub struct RodioBackend;

impl AudioBackend for RodioBackend {
    fn play_blocking(&self, samples: &[f32], rate: u32, channels: u16) -> Result<(), AdhanError> {
        // Open a stream to the system default output device.  There is no
        // device-selection logic here by design — routing is delegated to the
        // platform audio service.
        let (_stream, stream_handle) =
            OutputStream::try_default().map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;

        let sink = Sink::try_new(&stream_handle).map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;

        // Wrap the raw samples in a rodio `SamplesBuffer` so the sink can
        // consume them without any further file I/O or decoding.
        let source = rodio::buffer::SamplesBuffer::new(channels, rate, samples.to_vec());
        sink.append(source);

        // Block the calling thread until the sink has drained completely.
        sink.sleep_until_end();

        Ok(())
    }
}
