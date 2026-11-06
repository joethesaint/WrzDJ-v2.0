//! The rotating `bridge.log`, ported from `log-file-writer.ts`.
//!
//! Lines are `TIMESTAMP [LEVEL] message`, the timestamp as JavaScript's `toISOString()`. At 1 MiB
//! the file moves to `bridge.log.1` (one backup, overwritten). Write errors are swallowed:
//! logging never stops the Bridge.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_FILE_SIZE_BYTES: u64 = 1024 * 1024;
pub const LOG_FILENAME: &str = "bridge.log";
pub const BACKUP_FILENAME: &str = "bridge.log.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn upper(self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

#[derive(Debug)]
pub struct LogFileWriter {
    log_path: PathBuf,
    backup_path: PathBuf,
    current_size: u64,
    initialized: bool,
}

impl LogFileWriter {
    pub fn new(logs_dir: impl AsRef<Path>) -> Self {
        let dir = logs_dir.as_ref();
        LogFileWriter {
            log_path: dir.join(LOG_FILENAME),
            backup_path: dir.join(BACKUP_FILENAME),
            current_size: 0,
            initialized: false,
        }
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    pub fn write(&mut self, level: LogLevel, message: &str) {
        if !self.initialized {
            self.initialize();
        }
        let line = format!("{} [{}] {message}\n", iso8601_now(), level.upper());
        let written = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .and_then(|mut f| f.write_all(line.as_bytes()));
        if written.is_ok() {
            self.current_size += line.len() as u64;
            if self.current_size >= MAX_FILE_SIZE_BYTES
                && fs::rename(&self.log_path, &self.backup_path).is_ok()
            {
                self.current_size = 0;
            }
        }
    }

    fn initialize(&mut self) {
        self.initialized = true;
        if let Some(dir) = self.log_path.parent()
            && fs::create_dir_all(dir).is_err()
        {
            return;
        }
        self.current_size = fs::metadata(&self.log_path).map(|m| m.len()).unwrap_or(0);
    }
}

pub fn iso8601_now() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    iso8601(ms)
}

/// `new Date(ms).toISOString()` for ms since 1970 (UTC, millisecond precision).
pub fn iso8601(ms: u64) -> String {
    let days = (ms / 86_400_000) as i64;
    let rem = ms % 86_400_000;
    // Days to civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3_600_000,
        rem / 60_000 % 60,
        rem / 1_000 % 60,
        rem % 1_000
    )
}

#[cfg(test)]
pub(crate) fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    std::env::temp_dir().join(format!(
        "wrzdj-{tag}-{}-{nanos}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_log_entries_to_file() {
        let dir = temp_dir("log");
        let mut w = LogFileWriter::new(&dir);
        w.write(LogLevel::Info, "Bridge started");
        let text = fs::read_to_string(w.log_path()).unwrap();
        assert!(text.ends_with(" [INFO] Bridge started\n"), "{text}");
        assert_eq!(text.as_bytes()[10], b'T');
        assert!(text[..24].ends_with('Z'));
    }

    #[test]
    fn creates_the_logs_directory() {
        let dir = temp_dir("log").join("nested").join("logs");
        LogFileWriter::new(&dir).write(LogLevel::Warn, "x");
        assert!(dir.join(LOG_FILENAME).exists());
    }

    #[test]
    fn rotates_past_one_mebibyte_keeping_one_backup() {
        let dir = temp_dir("log");
        let mut w = LogFileWriter::new(&dir);
        let big = "x".repeat(10_000);
        for _ in 0..110 {
            w.write(LogLevel::Debug, &big);
        }
        let backup = fs::metadata(dir.join(BACKUP_FILENAME)).unwrap().len();
        let current = fs::metadata(w.log_path()).unwrap().len();
        assert!(backup >= MAX_FILE_SIZE_BYTES, "backup {backup}");
        assert!(current < MAX_FILE_SIZE_BYTES, "current {current}");
    }

    #[test]
    fn counts_an_existing_file_toward_rotation() {
        let dir = temp_dir("log");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(LOG_FILENAME),
            vec![b'a'; MAX_FILE_SIZE_BYTES as usize - 10],
        )
        .unwrap();
        LogFileWriter::new(&dir).write(LogLevel::Info, "tips it over");
        assert!(dir.join(BACKUP_FILENAME).exists());
    }

    #[test]
    fn log_path_and_all_levels() {
        let dir = temp_dir("log");
        let mut w = LogFileWriter::new(&dir);
        assert_eq!(w.log_path(), dir.join("bridge.log"));
        for level in [
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ] {
            w.write(level, "m");
        }
        let text = fs::read_to_string(w.log_path()).unwrap();
        for tag in ["[DEBUG]", "[INFO]", "[WARN]", "[ERROR]"] {
            assert!(text.contains(tag), "{tag}");
        }
    }

    #[test]
    fn unwritable_location_does_not_panic() {
        let dir = temp_dir("log");
        fs::write(&dir, b"a file, not a directory").unwrap();
        LogFileWriter::new(&dir).write(LogLevel::Error, "dropped");
    }

    #[test]
    fn iso8601_matches_to_iso_string() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso8601(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(iso8601(1_762_404_123_456), "2025-11-06T04:42:03.456Z");
    }
}
