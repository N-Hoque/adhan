use rodio::{DeviceSinkBuilder, Player};

use crate::{backend::AudioBackend, model::AdhanError};

/// Audio backend that uses `rodio` for playback.
///
/// Used on macOS and Windows. Wraps rodio's `OutputStream` + `Sink` pair
/// and blocks until the sink drains, matching the semantics expected by
/// `play_adhan`.
///
/// The source is streamed lazily — rodio pulls samples from the iterator as
/// it needs them, so no full decode or heap copy occurs before playback starts.
pub struct RodioBackend;

impl AudioBackend for RodioBackend {
    fn play_blocking(&self, source: Box<dyn rodio::Source<Item = f32> + Send>) -> Result<(), AdhanError> {
        // Open a stream to the system default output device.  There is no
        // device-selection logic here by design — routing is delegated to the
        // platform audio service.
        let stream = DeviceSinkBuilder::open_default_sink().map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;
        let mixer = stream.mixer();

        let player = Player::connect_new(mixer);

        // Append the source directly — rodio pulls samples lazily as the sink
        // drains, so no pre-decode or allocation is needed here.
        player.append(source);

        // Block the calling thread until the sink has drained completely.
        player.sleep_until_end();

        Ok(())
    }
}
