//! The Electron app's background checks (`bridge-runner.ts`), for the native shell:
//! - [`HealthWatcher`]: every 30 s, is the event still there? Deleted or expired stops the
//!   Bridge ("Event was deleted" / "Event expired or archived"); a failed check never does.
//! - [`network_warnings`]: the shared-subnet warning, logged at start.
//!
//! Beyond the TypeScript: a failed check is recorded (`health:check`, a warning) instead of
//! passing silently, so a run that lost the server shows it in the run log.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::api::{EventHealth, check_event_health};
use super::netcheck::{detect_subnet_conflicts, format_conflict_warnings};
use super::runlog::RunLog;

/// `HEALTH_CHECK_INTERVAL_MS` in `bridge-runner.ts`.
pub const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    EventDeleted,
    EventExpired,
}

impl StopReason {
    /// The reason `bridge-runner.ts` passes to `stop()`.
    pub fn message(self) -> &'static str {
        match self {
            StopReason::EventDeleted => "Event was deleted",
            StopReason::EventExpired => "Event expired or archived",
        }
    }
}

pub struct HealthWatcher {
    stop_tx: Option<mpsc::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl HealthWatcher {
    /// Check `event_code` every `interval` (first check after one interval, as `setInterval`).
    /// `on_stop` runs once, on the watcher's thread, when the event is deleted or expired.
    pub fn spawn(
        api_url: &str,
        event_code: &str,
        interval: Duration,
        log: Option<&'static RunLog>,
        on_stop: impl FnOnce(StopReason) + Send + 'static,
    ) -> std::io::Result<Self> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (api_url, event_code) = (api_url.to_string(), event_code.to_string());
        let handle = thread::Builder::new()
            .name("health-watch".into())
            .spawn(move || {
                loop {
                    match stop_rx.recv_timeout(interval) {
                        Err(RecvTimeoutError::Timeout) => {}
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
                    }
                    let reason = match check_event_health(&api_url, &event_code) {
                        EventHealth::Active => continue,
                        EventHealth::NotFound => StopReason::EventDeleted,
                        EventHealth::Expired => StopReason::EventExpired,
                        EventHealth::Error => {
                            if let Some(log) = log {
                                log.warn(
                                    "health:check",
                                    format!("could not check event {event_code}; will retry"),
                                );
                            }
                            continue;
                        }
                    };
                    if let Some(log) = log {
                        log.error(
                            "health:check",
                            format!("{} — stopping the Bridge", reason.message()),
                        );
                    }
                    on_stop(reason);
                    return;
                }
            })?;
        Ok(HealthWatcher {
            stop_tx: Some(stop_tx),
            handle: Some(handle),
        })
    }

    /// Stop checking and wait for the thread.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.handle.take()
            && handle.join().is_err()
            && let Some(log) = super::runlog::global()
        {
            log.error("health:check", "the watcher thread panicked");
        }
    }
}

impl Drop for HealthWatcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// The shared-subnet warnings, each also recorded as a `network:subnet` warning.
pub fn network_warnings(log: Option<&'static RunLog>) -> Vec<String> {
    let warnings = format_conflict_warnings(&detect_subnet_conflicts());
    if let Some(log) = log {
        for w in &warnings {
            log.warn("network:subnet", w);
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::logfile::temp_dir;
    use crate::shell::mock::{Reply as R, serve};
    use std::time::Instant;

    const TICK: Duration = Duration::from_millis(40);

    fn leaked_log() -> &'static RunLog {
        let dir = temp_dir("watch");
        let log = Box::leak(Box::new(RunLog::with_paths(
            dir.join("errors.log"),
            dir.join("errors.jsonl"),
        )));
        log.set_echo(false);
        log
    }

    fn watch(url: &str, log: &'static RunLog) -> (HealthWatcher, mpsc::Receiver<StopReason>) {
        let (tx, rx) = mpsc::channel();
        let w = HealthWatcher::spawn(url, "EVT 1", TICK, Some(log), move |r| {
            let _ = tx.send(r);
        })
        .unwrap();
        (w, rx)
    }

    #[test]
    fn deleted_event_stops_the_bridge() {
        let (url, seen) = serve(vec![R::Status(200, "{}"), R::Status(404, "")]);
        let log = leaked_log();
        let (_w, rx) = watch(&url, log);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            StopReason::EventDeleted
        );
        assert_eq!(seen.recv().unwrap().path, "/api/public/events/EVT%201");
        assert_eq!(StopReason::EventDeleted.message(), "Event was deleted");
        assert!(
            std::fs::read_to_string(log.log_path())
                .unwrap()
                .contains("ERROR health:check — Event was deleted")
        );
    }

    #[test]
    fn expired_event_stops_the_bridge() {
        let (url, _s) = serve(vec![R::Status(410, "")]);
        let (_w, rx) = watch(&url, leaked_log());
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            StopReason::EventExpired
        );
        assert_eq!(
            StopReason::EventExpired.message(),
            "Event expired or archived"
        );
    }

    #[test]
    fn a_failed_check_never_stops_it_but_is_logged() {
        let (url, _s) = serve(vec![
            R::Status(500, ""),
            R::Close,
            R::Status(200, "{}"),
            R::Status(404, ""),
        ]);
        let log = leaked_log();
        let (_w, rx) = watch(&url, log);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            StopReason::EventDeleted
        );
        assert_eq!(
            log.issue_count(),
            3,
            "two failed checks (warn) + the stop (error)"
        );
    }

    #[test]
    fn stop_ends_the_watcher_before_any_check() {
        let (url, seen) = serve(vec![R::Status(404, "")]);
        let (tx, rx) = mpsc::channel();
        let w = HealthWatcher::spawn(&url, "E", Duration::from_secs(30), None, move |r| {
            let _ = tx.send(r);
        })
        .unwrap();
        let started = Instant::now();
        w.stop();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(rx.try_recv().is_err());
        assert!(seen.try_recv().is_err(), "no request was made");
    }

    #[test]
    fn network_warnings_run() {
        let _ = network_warnings(Some(leaked_log()));
    }
}
