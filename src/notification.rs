use notify_rust::{Notification, Timeout};
use salah::Event;

use crate::{model::AdhanError, schedule::PrayerEventHandler};

/// A `PrayerEventHandler` that sends a desktop notification when a prayer
/// time arrives.
///
/// Uses `notify-rust` which abstracts over the platform notification APIs:
/// D-Bus / XDG on Linux, `NSUserNotification` on macOS, and WinRT toasts on
/// Windows. The same code path is used on all platforms.
///
/// # Failure behaviour
///
/// Notification delivery is best-effort. If the notification daemon is not
/// running (e.g. a headless Pi with no desktop session), `show()` will
/// return an error. The scheduler logs the error and continues — audio
/// playback is unaffected.
pub struct PrayerNotifier;

impl PrayerEventHandler for PrayerNotifier {
    fn on_prayer(&self, event: &Event, event_name: &str) -> Result<(), AdhanError> {
        // Non-prayer events are skipped — consistent with AdhanPlayer's
        // behaviour so the notification and audio remain in sync.
        if !matches!(event, Event::Prayer(_)) {
            return Ok(());
        }

        Notification::new()
            .appname("adhan")
            .summary(event_name)
            .body(&format!("It is now time for {}", event_name))
            .timeout(Timeout::Milliseconds(8_000))
            .show()
            .map_err(|e| AdhanError::Notification(e.to_string()))?;

        Ok(())
    }
}
