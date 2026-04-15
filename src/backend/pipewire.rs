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
    stream::{StreamBox, StreamFlags, StreamListener},
};
use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapCons, HeapRb,
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
/// An `Arc<AtomicBool>` done flag is set by the feeder once the source is
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
        let (producer, consumer) = rb.split();

        let feeder_done = Arc::new(std::sync::atomic::AtomicBool::new(false));

        spawn_feeder_thread(source, producer, Arc::clone(&feeder_done));

        // ── PipeWire session setup ────────────────────────────────────────
        //
        // The three mandatory PipeWire objects must be created in this order
        // and kept alive for the duration of the session. CoreBox and
        // StreamBox carry lifetimes tied to their parents so they are kept
        // local to this frame rather than returned from helper functions.
        let mainloop = MainLoopRc::new(None).map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;
        let context = ContextBox::new(mainloop.loop_(), None).map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;
        let core = context
            .connect(None)
            .map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;

        // ── Stream creation ───────────────────────────────────────────────
        //
        // Registers this process as a named music playback client. Properties
        // are set here, before the listener is attached, so they are readable
        // independently of the callback logic below.
        let stream = StreamBox::new(
            &core,
            "adhan",
            properties! {
                *pipewire::keys::MEDIA_TYPE     => "Audio",
                *pipewire::keys::MEDIA_ROLE     => "Music",
                *pipewire::keys::MEDIA_CATEGORY => "Playback",
                *pipewire::keys::APP_NAME       => "Adhan Player",
                *pipewire::keys::APP_ID         => "nhoque.adhan",
            },
        )
        .map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;

        // ── Process callback ──────────────────────────────────────────────
        let finished = register_process_callback(&stream, consumer, feeder_done, mainloop.clone(), channels.into())?;

        // ── Format negotiation + connect ──────────────────────────────────
        let format_pod_bytes = build_format_pod(rate.into(), channels.into()).map_err(AdhanError::AudioPlayback)?;
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

        // ── Run until the process callback signals completion ─────────────
        mainloop.run();

        if !finished.get() {
            return Err(AdhanError::AudioPlayback(
                "PipeWire main loop exited before playback completed".into(),
            ));
        }

        Ok(())
    }
}

// ── Feeder thread ─────────────────────────────────────────────────────────────

/// Spawns a thread that drains `source` and pushes f32 samples into `producer`
/// in fixed-size chunks.
///
/// The feeder has no dependency on PipeWire — it only knows about the ring
/// buffer and the done flag. This makes it independently understandable and
/// keeps the chunked-write logic separate from the PipeWire session code.
///
/// Chunk size is 512 samples — at 48 kHz stereo that is ~5 ms of audio,
/// matching a typical PipeWire quantum. Large enough to amortise loop
/// overhead, small enough not to stall the feeder waiting to fill it.
///
/// When the source is exhausted, `feeder_done` is set to `true` so the
/// process callback knows it can quit once the ring buffer has been drained.
fn spawn_feeder_thread(
    mut source: Box<dyn rodio::Source<Item = f32> + Send>,
    mut producer: ringbuf::HeapProd<f32>,
    feeder_done: Arc<std::sync::atomic::AtomicBool>,
) {
    thread::spawn(move || {
        const CHUNK: usize = 512;
        let mut buf = Vec::with_capacity(CHUNK);

        'outer: loop {
            buf.clear();
            for sample in source.by_ref().take(CHUNK) {
                buf.push(sample);
            }

            // Source exhausted and nothing buffered — we're done.
            if buf.is_empty() {
                break 'outer;
            }

            // Push the chunk into the ring buffer. If there isn't enough
            // space, sleep briefly and retry rather than spinning hard.
            let mut written = 0;
            while written < buf.len() {
                let n = producer.push_slice(&buf[written..]);
                written += n;
                if written < buf.len() {
                    thread::sleep(std::time::Duration::from_millis(1));
                }
            }

            // Fewer samples than requested means the source is exhausted.
            if buf.len() < CHUNK {
                break 'outer;
            }
        }

        feeder_done.store(true, std::sync::atomic::Ordering::SeqCst);
    });
}

// ── Process callback ──────────────────────────────────────────────────────────

/// Registers the real-time `process` callback on `stream` and returns the
/// `finished` flag that will be set to `true` once playback is complete.
///
/// # Shared state
///
/// The callback runs on the PipeWire main loop thread, so `Rc`/`RefCell` are
/// sufficient for interior mutability — no mutex needed. The `feeder_done`
/// flag crosses the thread boundary from the feeder thread and therefore uses
/// `Arc<AtomicBool>`.
///
/// # Real-time contract
///
/// The callback must not allocate, block, or perform I/O. It only:
/// - pops samples from the lock-free ring buffer into PipeWire's buffer,
/// - zeros any unfilled frames (underrun guard),
/// - sets chunk metadata (offset / size / stride),
/// - quits the main loop once the feeder is done and the buffer is empty.
///
/// # Listener lifetime
///
/// The returned `finished` flag must be checked only after `mainloop.run()`
/// returns. The listener is leaked via `mem::forget` so it remains registered
/// for the entire duration of the main loop run, and is reclaimed when the
/// enclosing `play_blocking` stack frame unwinds.
fn register_process_callback(
    stream: &StreamBox,
    consumer: HeapCons<f32>,
    feeder_done: Arc<std::sync::atomic::AtomicBool>,
    quit_loop: MainLoopRc,
    channels: u16,
) -> Result<Rc<Cell<bool>>, AdhanError> {
    let consumer_cell = Rc::new(std::cell::RefCell::new(consumer));
    let finished = Rc::new(Cell::new(false));

    let consumer_cb = Rc::clone(&consumer_cell);
    let finished_cb = Rc::clone(&finished);

    let listener: StreamListener<()> = stream
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

                let mut cons = consumer_cb.borrow_mut();
                let n = cons.pop_slice(dst);

                // Zero any frames we couldn't fill — prevents stale audio
                // from a previous callback invocation leaking through.
                dst[n..].fill(0.0);

                let byte_count = (n * std::mem::size_of::<f32>()) as u32;

                // We're done when the feeder has finished AND the ring buffer
                // is empty — every decoded sample has been handed to PipeWire.
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

    // Keep the listener alive for the full duration of mainloop.run().
    // It will be reclaimed when play_blocking's stack frame unwinds.
    std::mem::forget(listener);

    Ok(finished)
}

// ── Format Pod builder ────────────────────────────────────────────────────────

/// Serialises an `EnumFormat` SPA Object Pod describing interleaved f32 PCM.
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
