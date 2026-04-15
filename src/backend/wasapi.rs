use std::process;

use rodio::{DeviceSinkBuilder, Player};
use windows::{
    core::{Interface, GUID},
    Win32::{
        Media::Audio::{
            eConsole, eRender, AudioSessionStateActive, IAudioSessionControl, IAudioSessionControl2,
            IAudioSessionEnumerator, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume,
            MMDeviceEnumerator,
        },
        System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED},
    },
};

use crate::{backend::AudioBackend, model::AdhanError};

/// Audio backend for Windows that uses WASAPI session management to mute all
/// other active audio streams before playback, then restores them afterwards.
///
/// # Ducking strategy
///
/// Before the adhan plays, [`WasapiBackend`] enumerates every active
/// `IAudioSession` on the default render endpoint and calls
/// `ISimpleAudioVolume::SetMute(TRUE)` on each one, skipping:
///
/// - Our own process (matched by PID via `IAudioSessionControl2::GetProcessId`)
/// - Sessions that are not currently active (`AudioSessionStateActive`)
///
/// After playback completes — whether successfully or with an error — every
/// session that was muted by us is restored with `SetMute(FALSE)`.
///
/// `SetMute` is used rather than `SetMasterVolume(0.0)` because mute and
/// volume are orthogonal: restoring is a simple `SetMute(FALSE)` with no need
/// to track prior volume levels, and a crash before restore does not
/// permanently alter the user's volume settings.
///
/// # Playback
///
/// Actual audio output is delegated to rodio, exactly as `RodioBackend` does.
/// The value of this backend over `RodioBackend` is entirely the session
/// ducking; there is no need to reimplement the playback path.
///
/// # COM threading
///
/// Each call to `play_blocking` initialises COM on the calling thread with
/// `COINIT_APARTMENTTHREADED` (STA). This is appropriate for a synchronous,
/// single-threaded operation. `CoUninitialize` is called via a RAII guard when
/// the function returns, regardless of success or failure.
pub struct WasapiBackend;

impl AudioBackend for WasapiBackend {
    fn play_blocking(&self, source: Box<dyn rodio::Source<Item = f32> + Send>) -> Result<(), AdhanError> {
        // Initialise COM for this thread. The guard calls CoUninitialize on drop.
        let _com = ComGuard::init()?;

        // Mute all other active sessions. If enumeration fails we log a warning
        // and proceed with playback unducked rather than refusing to play.
        let muted = match duck_other_sessions() {
            Ok(sessions) => sessions,
            Err(e) => {
                log::warn!("WASAPI session ducking failed, playing without ducking: {e}");
                vec![]
            }
        };

        // Play the adhan via rodio. Capture the result so we can restore
        // sessions before propagating any error.
        let play_result = play_with_rodio(source);

        // Restore all muted sessions regardless of playback outcome.
        restore_sessions(&muted);

        play_result
    }
}

// ── COM RAII guard ────────────────────────────────────────────────────────────

/// Calls `CoInitializeEx` on construction and `CoUninitialize` on drop,
/// ensuring COM is always torn down even if the calling function returns early.
struct ComGuard;

impl ComGuard {
    fn init() -> Result<Self, AdhanError> {
        // S_OK  = COM initialised successfully for this thread.
        // RPC_E_CHANGED_MODE (0x80010106) would mean the thread already has COM
        // initialised with a different model — treat that as success since COM
        // is still usable; we just won't uninitialise on drop in that case.
        // Any other error is fatal.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr.is_err() && hr != windows::Win32::Foundation::RPC_E_CHANGED_MODE {
            return Err(AdhanError::AudioPlayback(format!("CoInitializeEx failed: {hr:?}")));
        }
        Ok(ComGuard)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

// ── Session ducking ───────────────────────────────────────────────────────────

/// Enumerates all active audio sessions on the default render endpoint,
/// mutes every session that does not belong to our process, and returns the
/// `ISimpleAudioVolume` interfaces of the sessions that were muted so they
/// can be restored later.
///
/// Errors from individual sessions are logged and skipped rather than
/// aborting the entire enumeration — a failure to mute one session should not
/// prevent the adhan from playing.
fn duck_other_sessions() -> Result<Vec<ISimpleAudioVolume>, AdhanError> {
    let our_pid = process::id();

    // ── Obtain the default render endpoint ───────────────────────────────────
    let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
        .map_err(|e| AdhanError::AudioPlayback(format!("CoCreateInstance(MMDeviceEnumerator): {e}")))?;

    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
        .map_err(|e| AdhanError::AudioPlayback(format!("GetDefaultAudioEndpoint: {e}")))?;

    // ── Obtain the session manager ────────────────────────────────────────────
    //
    // IMMDevice::Activate is generic over the COM interface type in windows 0.59+.
    // The type annotation on `session_manager` drives which interface GUID is
    // passed to the underlying COM call.
    let session_manager: IAudioSessionManager2 = unsafe { device.Activate(CLSCTX_ALL, None) }
        .map_err(|e| AdhanError::AudioPlayback(format!("IMMDevice::Activate(IAudioSessionManager2): {e}")))?;

    // ── Enumerate sessions ────────────────────────────────────────────────────
    //
    // GetSessionEnumerator returns a point-in-time snapshot. Sessions that open
    // after this call are not included — they will play at full volume, which
    // is acceptable since they opened after the adhan started.
    let session_enum: IAudioSessionEnumerator = unsafe { session_manager.GetSessionEnumerator() }
        .map_err(|e| AdhanError::AudioPlayback(format!("GetSessionEnumerator: {e}")))?;

    let count = unsafe { session_enum.GetCount() }
        .map_err(|e| AdhanError::AudioPlayback(format!("IAudioSessionEnumerator::GetCount: {e}")))?;

    let mut muted: Vec<ISimpleAudioVolume> = Vec::new();

    for i in 0..count {
        // Retrieve the session control. A failure on a single session is not
        // fatal — log and continue to the next.
        let session_ctrl: IAudioSessionControl = match unsafe { session_enum.GetSession(i) } {
            Ok(s) => s,
            Err(e) => {
                log::debug!("GetSession({i}) failed, skipping: {e}");
                continue;
            }
        };

        // ── Skip inactive sessions ────────────────────────────────────────────
        let state = match unsafe { session_ctrl.GetState() } {
            Ok(s) => s,
            Err(e) => {
                log::debug!("GetState for session {i} failed, skipping: {e}");
                continue;
            }
        };

        if state != AudioSessionStateActive {
            continue;
        }

        // ── Skip our own process ──────────────────────────────────────────────
        let ctrl2: IAudioSessionControl2 = match session_ctrl.cast() {
            Ok(c) => c,
            Err(e) => {
                log::debug!("QI to IAudioSessionControl2 for session {i} failed, skipping: {e}");
                continue;
            }
        };

        let pid = match unsafe { ctrl2.GetProcessId() } {
            Ok(p) => p,
            Err(e) => {
                log::debug!("GetProcessId for session {i} failed, skipping: {e}");
                continue;
            }
        };

        if pid == our_pid {
            continue;
        }

        // ── Mute the session ──────────────────────────────────────────────────
        let volume: ISimpleAudioVolume = match session_ctrl.cast() {
            Ok(v) => v,
            Err(e) => {
                log::debug!("QI to ISimpleAudioVolume for session {i} failed, skipping: {e}");
                continue;
            }
        };

        match unsafe { volume.SetMute(true, &GUID::zeroed()) } {
            Ok(()) => {
                log::debug!("Muted audio session {i} (PID {pid})");
                muted.push(volume);
            }
            Err(e) => {
                log::debug!("SetMute for session {i} (PID {pid}) failed, skipping: {e}");
            }
        }
    }

    log::debug!("Ducked {} session(s) before adhan playback", muted.len());
    Ok(muted)
}

/// Restores all sessions that were muted by [`duck_other_sessions`].
///
/// Failures to restore individual sessions are logged but do not propagate —
/// best-effort restoration is correct here because playback has already
/// completed and there is nothing useful to do with an error at this point.
fn restore_sessions(sessions: &[ISimpleAudioVolume]) {
    for (i, volume) in sessions.iter().enumerate() {
        match unsafe { volume.SetMute(false, &GUID::zeroed()) } {
            Ok(()) => log::debug!("Restored audio session {i}"),
            Err(e) => log::warn!("Failed to restore audio session {i}: {e}"),
        }
    }
    log::debug!("Restored {} session(s) after adhan playback", sessions.len());
}

// ── Rodio playback ────────────────────────────────────────────────────────────

/// Plays `source` to the default audio output device using rodio.
///
/// This is identical to `RodioBackend::play_blocking` and is extracted here
/// so the ducking logic and playback logic remain separately readable.
fn play_with_rodio(source: Box<dyn rodio::Source<Item = f32> + Send>) -> Result<(), AdhanError> {
    let stream = DeviceSinkBuilder::open_default_sink().map_err(|e| AdhanError::AudioPlayback(e.to_string()))?;
    let mixer = stream.mixer();
    let player = Player::connect_new(mixer);
    player.append(source);
    player.sleep_until_end();
    Ok(())
}
