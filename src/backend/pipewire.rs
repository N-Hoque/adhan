use std::{cell::Cell, rc::Rc, sync::Arc, thread};

use pipewire::spa::{
    param::audio::{AudioFormat, AudioInfoRaw},
    pod::{serialize::PodSerializer, Object, Pod, Property, PropertyFlags, Value},
    sys::{
        SPA_FORMAT_AUDIO_channels, SPA_FORMAT_AUDIO_format, SPA_FORMAT_AUDIO_rate, SPA_FORMAT_mediaSubtype,
        SPA_FORMAT_mediaType, SPA_MEDIA_SUBTYPE_raw, SPA_MEDIA_TYPE_audio, SPA_PARAM_EnumFormat,
        SPA_TYPE_OBJECT_Format,
    },
    utils::{Direction, Id},
};
use pipewire::{
    context::ContextBox,
    main_loop::MainLoopRc,
    properties::properties,
    stream::{StreamBox, StreamFlags},
};
use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapRb,
};

use crate::{backend::AudioBackend, model::AdhanError};

/// Capacity of the ring buffer in f32 samples.
///
/// At 48 kHz stereo this is ~170 ms of audio — enough headroom to absorb
/// scheduling jitter between the feeder thread and the PipeWire process
/// callback without starving the stream, while keeping memory use modest.
const RING_BUFFER_CAPACITY: usize = 16_384;

/// Audio backend that plays through the running PipeWire session.
///
/// The binary registers itself as a named PipeWire client ("adhan"), so the
/// stream appears in any PipeWire patchbay tool (e.g. `qpwgraph`, `helvum`,
/// `pavucontrol`) and can be routed, volume-adjusted, or recorded without
/// touching this binary.
///
/// # Architecture
///
/// Playback uses a double-buffer design to keep the PipeWire real-time
/// `process` callback allocation- and I/O-free:
///
/// ```text
///  ┌─────────────────┐   f32 samples   ┌──────────────────────┐
///  │  feeder thread  │ ──────────────► │  lock-free ring buf  │
///  │  (MP3 decoder)  │                 └──────────┬───────────┘
///  └─────────────────┘                            │ pop_slice
///                                      ┌──────────▼───────────┐
///                                      │  PW process callback │
///                                      │  (real-time thread)  │
///                                      └──────────────────────┘
/// ```
///
/// The feeder thread drains the `Source` iterator and pushes decoded f32
/// samples into the producer half of a `ringbuf::HeapRb`. The PipeWire
/// `process` callback holds the consumer half and copies samples out into
/// PipeWire's buffer on demand — no allocation, no I/O, no blocking.
///
/// An `Arc<atomic>` done flag is set by the feeder once the source is
/// exhausted. The process callback drains any remaining samples from the
/// ring buffer and then quits the main loop.
pub struct PipewireBackend;

impl AudioBackend for PipewireBackend {
    fn play_blocking(&self, source: Box<dyn rodio::Source<Item = f32> + Send>) -> Result<(), AdhanError> {
        let rate = source.sample_rate();
        let channels = source.channels();

        // ── Ring buffer ───────────────────────────────────────────────────
        //
        // Split into (producer, consumer). The producer is moved into the
        // feeder thread; the consumer is moved into the process callback.
        // HeapRb is lock-free and safe to use across threads.

        let rb = HeapRb::<f32>::new(RING_BUFFER_CAPACITY);
        let (mut producer, consumer) = rb.split();

        // Shared flag: feeder sets this to true once the source iterator
        // is exhausted. Uses SeqCst ordering for simplicity — this is not
        // on the hot path.
        let feeder_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let feeder_done_feeder = Arc::clone(&feeder_done);

        // ── Feeder thread ─────────────────────────────────────────────────
        //
        // Drains the Source iterator and pushes samples into the ring buffer.
        // Blocks (yields) when the ring buffer is full so we don't spin-waste
        // a full core — the process callback will drain it shortly.

        thread::spawn(move || {
            for sample in source {
                // Push one sample at a time. If the ring buffer is full,
                // spin-yield until there is space. This keeps the feeder
                // in lock-step with playback without busy-waiting hard.
                while producer.try_push(sample).is_err() {
                    thread::yield_now();
                }
            }
            feeder_done_feeder.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        // ── PipeWire setup ────────────────────────────────────────────────

        let mainloop = MainLoopRc::new(None).map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;

        let context = ContextBox::new(mainloop.loop_(), None).map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;

        let core = context
            .connect(None)
            .map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;

        // ── Stream ────────────────────────────────────────────────────────

        let stream = StreamBox::new(
            &core,
            "adhan",
            properties! {
                *pipewire::keys::MEDIA_TYPE     => "Audio",
                *pipewire::keys::MEDIA_ROLE     => "Music",
                *pipewire::keys::MEDIA_CATEGORY => "Playback",
                *pipewire::keys::APP_NAME       => "adhan",
                *pipewire::keys::APP_ID         => "adhan",
            },
        )
        .map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;

        // ── Shared state for the process callback ─────────────────────────
        //
        // Everything in the process callback must be Send + 'static.
        // The ring buffer consumer is already Send. We wrap it in an
        // Rc<RefCell<_>> because the callback closure is not Send itself
        // (PipeWire callbacks run on the same thread as the main loop),
        // but we need interior mutability to advance the consumer.

        let consumer_cell = Rc::new(std::cell::RefCell::new(consumer));
        let finished = Rc::new(Cell::new(false));

        let consumer_cb = Rc::clone(&consumer_cell);
        let finished_cb = Rc::clone(&finished);
        let quit_loop = mainloop.clone();

        // ── Listener / callbacks ──────────────────────────────────────────

        let _listener = stream
            .add_local_listener::<()>()
            .process(move |stream, _userdata| {
                let Some(mut buf) = stream.dequeue_buffer() else {
                    return;
                };

                let datas = buf.datas_mut();
                let data = &mut datas[0];

                let (byte_count, should_quit) = {
                    let dst_raw: &mut [u8] = match data.data() {
                        Some(d) => d,
                        None => return,
                    };

                    let max_samples = dst_raw.len() / std::mem::size_of::<f32>();
                    let dst: &mut [f32] =
                        bytemuck::cast_slice_mut(&mut dst_raw[..max_samples * std::mem::size_of::<f32>()]);

                    // Pop as many samples as will fit in this PipeWire buffer.
                    let mut cons = consumer_cb.borrow_mut();
                    let n = cons.pop_slice(dst);

                    // Zero out any frames we couldn't fill (underrun guard).
                    dst[n..].fill(0.0);

                    let byte_count = (n * std::mem::size_of::<f32>()) as u32;

                    // We're done when the feeder has finished AND the ring
                    // buffer is now empty (all samples have been consumed).
                    let done = feeder_done.load(std::sync::atomic::Ordering::SeqCst) && cons.is_empty();

                    (byte_count, done)
                };

                let chunk = data.chunk_mut();
                *chunk.offset_mut() = 0;
                *chunk.size_mut() = byte_count;
                *chunk.stride_mut() = (channels as i32) * (std::mem::size_of::<f32>() as i32);

                if should_quit {
                    finished_cb.set(true);
                    let _ = stream.flush(true);
                    quit_loop.quit();
                }
            })
            .register()
            .map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;

        // ── Format negotiation (SPA Pod) ──────────────────────────────────

        let format_pod_bytes = build_format_pod(rate, channels).map_err(|e| AdhanError::AudioPlayback(e))?;

        let format_pod = Pod::from_bytes(&format_pod_bytes)
            .ok_or_else(|| AdhanError::AudioPlayback("failed to construct SPA format pod".into()))?;

        stream
            .connect(
                Direction::Output,
                None,
                StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
                &mut [format_pod],
            )
            .map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;

        // ── Run until `process` signals completion ────────────────────────

        mainloop.run();

        if !finished.get() {
            return Err(AdhanError::AudioPlayback(
                "PipeWire main loop exited before playback completed".into(),
            ));
        }

        Ok(())
    }
}

// ── Format Pod builder ────────────────────────────────────────────────────────

/// Serialise an `EnumFormat` SPA Object Pod describing interleaved f32 PCM.
///
/// This is the mandatory parameter passed to `Stream::connect`. PipeWire uses
/// it to negotiate the audio format between our client and the chosen sink.
fn build_format_pod(rate: u32, channels: u16) -> Result<Vec<u8>, String> {
    let mut audio_info = AudioInfoRaw::new();
    audio_info.set_format(AudioFormat::F32LE);
    audio_info.set_rate(rate);
    audio_info.set_channels(channels as u32);

    let format_id: u32 = audio_info.format().as_raw();

    let value = Value::Object(Object {
        type_: SPA_TYPE_OBJECT_Format,
        id: SPA_PARAM_EnumFormat,
        properties: vec![
            Property {
                key: SPA_FORMAT_mediaType,
                flags: PropertyFlags::empty(),
                value: Value::Id(Id(SPA_MEDIA_TYPE_audio)),
            },
            Property {
                key: SPA_FORMAT_mediaSubtype,
                flags: PropertyFlags::empty(),
                value: Value::Id(Id(SPA_MEDIA_SUBTYPE_raw)),
            },
            Property {
                key: SPA_FORMAT_AUDIO_format,
                flags: PropertyFlags::empty(),
                value: Value::Id(Id(format_id)),
            },
            Property {
                key: SPA_FORMAT_AUDIO_rate,
                flags: PropertyFlags::empty(),
                value: Value::Int(rate as i32),
            },
            Property {
                key: SPA_FORMAT_AUDIO_channels,
                flags: PropertyFlags::empty(),
                value: Value::Int(channels as i32),
            },
        ],
    });

    let (serialized, _) =
        PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &value).map_err(|e| e.to_string())?;

    Ok(serialized.into_inner())
}
