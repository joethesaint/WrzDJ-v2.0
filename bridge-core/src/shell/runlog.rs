//! Never stop the run, always leave a record: the SYS-06 Run Log (`run-log.js`) in Rust.
//!
//! One capture point, two outputs:
//! - `errors.jsonl`, for agents: one JSON object per line,
//!   `{ ts, runId, level, context, message, stack, meta? }`. Group by `runId` to see one run.
//! - `errors.log`, for people: `[ts] LEVEL context — message`, no stack.
//!
//! Same formats and the same rules as the JavaScript: one run id per process, logging never
//! fails its caller, [`RunLog::run_safely`] catches and carries on, [`RunLog::summarize_run`]
//! gives the end-of-run line. Added for Rust:
//! - `run_safely` also catches panics.
//! - [`install_panic_hook`] records a panic on any thread.
//! - Writes are serialised, so threads never interleave lines.
//! - Each file is capped at [`MAX_FILE_BYTES`], with one `.1` backup. The JavaScript has no cap.
//!
//! Contexts are `<area>:<detail>`: `serato:read`, `client:post`, `shell:keychain`.

use std::cell::Cell;
use std::collections::hash_map::RandomState;
use std::fmt::Display;
use std::fs::{self, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};

use super::logfile::iso8601_now;

pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
pub const LOG_FILENAME: &str = "errors.log";
pub const JSONL_FILENAME: &str = "errors.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
        }
    }
}

/// `[ts] LEVEL context — first line of the message`.
pub fn format_human_line(ts: &str, level: Level, context: &str, message: &str) -> String {
    let first = message.split('\n').next().unwrap_or("");
    format!(
        "[{ts}] {} {context} — {first}",
        level.as_str().to_uppercase()
    )
}

/// One JSON object; `meta` only when given.
pub fn format_machine_entry(
    ts: &str,
    run_id: &str,
    level: Level,
    context: &str,
    message: &str,
    stack: Option<&str>,
    meta: Option<&Value>,
) -> String {
    let mut entry = json!({
        "ts": ts, "runId": run_id, "level": level.as_str(), "context": context,
        "message": message, "stack": stack,
    });
    if let Some(meta) = meta {
        entry["meta"] = meta.clone();
    }
    entry.to_string()
}

/// `2026-11-06T07-45-12-345Z_3fa9c1`, as `generateRunId()`.
pub fn new_run_id() -> String {
    let stamp = iso8601_now().replace([':', '.'], "-");
    let mut h = RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    h.write_u32(std::process::id());
    format!("{stamp}_{:06x}", h.finish() & 0xff_ffff)
}

/// The error and its causes, outermost first: Rust's nearest thing to a stack.
pub fn error_chain(err: &(dyn std::error::Error + 'static)) -> String {
    let mut parts = vec![err.to_string()];
    let mut source = err.source();
    while let Some(s) = source {
        parts.push(s.to_string());
        source = s.source();
    }
    parts.join("\ncaused by: ")
}

thread_local! {
    static IN_RUN_SAFELY: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug)]
pub struct RunLog {
    log_path: PathBuf,
    jsonl_path: PathBuf,
    run_id: String,
    issues: AtomicUsize,
    echo: AtomicBool,
    write_lock: Mutex<()>,
}

impl RunLog {
    /// `errors.log` and `errors.jsonl` in `dir`, unless `RUN_LOG_PATH` / `RUN_LOG_JSONL` say otherwise.
    pub fn new(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        let log =
            std::env::var_os("RUN_LOG_PATH").map_or_else(|| dir.join(LOG_FILENAME), PathBuf::from);
        let jsonl = std::env::var_os("RUN_LOG_JSONL")
            .map_or_else(|| dir.join(JSONL_FILENAME), PathBuf::from);
        Self::with_paths(log, jsonl)
    }

    pub fn with_paths(log_path: impl Into<PathBuf>, jsonl_path: impl Into<PathBuf>) -> Self {
        RunLog {
            log_path: log_path.into(),
            jsonl_path: jsonl_path.into(),
            run_id: new_run_id(),
            issues: AtomicUsize::new(0),
            echo: AtomicBool::new(true),
            write_lock: Mutex::new(()),
        }
    }

    /// Print each human line to stderr too (on by default, as `console.error`).
    pub fn set_echo(&self, on: bool) {
        self.echo.store(on, Ordering::Relaxed);
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    pub fn issue_count(&self) -> usize {
        self.issues.load(Ordering::Relaxed)
    }

    /// Record an issue in both outputs. Never fails. Returns the human line.
    pub fn log(
        &self,
        level: Level,
        context: &str,
        message: &str,
        stack: Option<&str>,
        meta: Option<&Value>,
    ) -> String {
        let ts = iso8601_now();
        let human = format_human_line(&ts, level, context, message);
        let machine = format_machine_entry(&ts, &self.run_id, level, context, message, stack, meta);
        if self.echo.load(Ordering::Relaxed) {
            eprintln!("{human}");
        }
        {
            let _guard = self.write_lock.lock().unwrap_or_else(|p| p.into_inner());
            append_line(&self.log_path, &human);
            append_line(&self.jsonl_path, &machine);
        }
        self.issues.fetch_add(1, Ordering::Relaxed);
        human
    }

    pub fn error(&self, context: &str, message: impl Display) -> String {
        self.log(Level::Error, context, &message.to_string(), None, None)
    }

    pub fn warn(&self, context: &str, message: impl Display) -> String {
        self.log(Level::Warn, context, &message.to_string(), None, None)
    }

    /// An error value, with its cause chain as the stack.
    pub fn error_value(
        &self,
        context: &str,
        err: &(dyn std::error::Error + 'static),
        meta: Option<&Value>,
    ) -> String {
        self.log(
            Level::Error,
            context,
            &err.to_string(),
            Some(&error_chain(err)),
            meta,
        )
    }

    /// Run `f`. On `Err` or a panic, record it and return `fallback`: the run carries on.
    pub fn run_safely<T, E: Display>(
        &self,
        context: &str,
        fallback: T,
        f: impl FnOnce() -> Result<T, E>,
    ) -> T {
        let outer = IN_RUN_SAFELY.with(|c| c.replace(true));
        let result = panic::catch_unwind(AssertUnwindSafe(f));
        IN_RUN_SAFELY.with(|c| c.set(outer));
        match result {
            Ok(Ok(value)) => value,
            Ok(Err(e)) => {
                self.error(context, e);
                fallback
            }
            Err(payload) => {
                let message = panic_message(payload.as_ref());
                self.log(
                    Level::Error,
                    context,
                    &format!("panic: {message}"),
                    None,
                    None,
                );
                fallback
            }
        }
    }

    /// `N issues this run — see <errors.log>`, or `None` when nothing went wrong.
    pub fn summarize_run(&self) -> Option<String> {
        match self.issue_count() {
            0 => None,
            n => Some(format!(
                "{n} {} this run — see {}",
                if n == 1 { "issue" } else { "issues" },
                self.log_path.display()
            )),
        }
    }
}

fn append_line(path: &Path, line: &str) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if fs::metadata(path).map(|m| m.len()).unwrap_or(0) >= MAX_FILE_BYTES {
        let mut backup = path.as_os_str().to_owned();
        backup.push(".1");
        let _ = fs::rename(path, backup);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".into())
}

static GLOBAL: OnceLock<RunLog> = OnceLock::new();

/// The process's run log, set once at start. Later calls return the first one.
pub fn init(dir: impl AsRef<Path>) -> &'static RunLog {
    GLOBAL.get_or_init(|| RunLog::new(dir))
}

pub fn global() -> Option<&'static RunLog> {
    GLOBAL.get()
}

/// Record panics on any thread in the global run log (context `panic:<thread>`, with the
/// file and line). Panics inside `run_safely` are left to it, so nothing is counted twice.
/// The previous hook still runs.
pub fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if !IN_RUN_SAFELY.with(Cell::get)
            && let Some(log) = global()
        {
            let thread = std::thread::current()
                .name()
                .unwrap_or("unnamed")
                .to_string();
            let location = info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()));
            let message = panic_message(info.payload());
            log.log(
                Level::Error,
                &format!("panic:{thread}"),
                &format!("panic: {message}"),
                location.as_deref(),
                None,
            );
        }
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::logfile::temp_dir;

    fn quiet(dir: &Path) -> RunLog {
        let log = RunLog::with_paths(dir.join(LOG_FILENAME), dir.join(JSONL_FILENAME));
        log.set_echo(false);
        log
    }

    #[test]
    fn human_line_matches_run_log_js() {
        let line = format_human_line(
            "2026-11-06T07:00:00.000Z",
            Level::Warn,
            "serato:read",
            "bad chunk\nmore detail",
        );
        assert_eq!(
            line,
            "[2026-11-06T07:00:00.000Z] WARN serato:read — bad chunk"
        );
    }

    #[test]
    fn machine_entry_has_the_js_keys_and_meta_only_when_given() {
        let plain: Value = serde_json::from_str(&format_machine_entry(
            "T",
            "R",
            Level::Error,
            "c",
            "m",
            None,
            None,
        ))
        .unwrap();
        assert_eq!(
            plain,
            json!({"ts":"T","runId":"R","level":"error","context":"c","message":"m","stack":null})
        );
        let with: Value = serde_json::from_str(&format_machine_entry(
            "T",
            "R",
            Level::Error,
            "c",
            "m",
            Some("s"),
            Some(&json!({"deck":1})),
        ))
        .unwrap();
        assert_eq!(with["stack"], "s");
        assert_eq!(with["meta"]["deck"], 1);
    }

    #[test]
    fn run_id_shape() {
        let id = new_run_id();
        let (stamp, rand) = id.split_once('_').unwrap();
        assert_eq!(stamp.len(), 24);
        assert!(!stamp.contains(':') && !stamp.contains('.'));
        assert_eq!(rand.len(), 6);
        assert!(rand.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn writes_both_outputs_with_one_run_id_and_counts_issues() {
        let dir = temp_dir("runlog");
        let log = quiet(&dir);
        log.error("client:post", "HTTP 500");
        log.warn("serato:read", "half-written chunk");
        let human = fs::read_to_string(dir.join(LOG_FILENAME)).unwrap();
        assert!(human.contains("ERROR client:post — HTTP 500"));
        assert!(human.contains("WARN serato:read — half-written chunk"));
        let entries: Vec<Value> = fs::read_to_string(dir.join(JSONL_FILENAME))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e["runId"] == log.run_id()));
        assert_eq!(log.issue_count(), 2);
    }

    #[test]
    fn run_safely_returns_the_value_or_logs_and_falls_back() {
        let dir = temp_dir("runlog");
        let log = quiet(&dir);
        assert_eq!(log.run_safely("ok", 0, || Ok::<_, String>(7)), 7);
        assert_eq!(
            log.run_safely("db:insert", -1, || Err::<i32, _>("disk full")),
            -1
        );
        assert_eq!(log.issue_count(), 1);
        assert!(
            fs::read_to_string(dir.join(LOG_FILENAME))
                .unwrap()
                .contains("db:insert — disk full")
        );
    }

    #[test]
    fn run_safely_catches_a_panic() {
        let dir = temp_dir("runlog");
        let log = quiet(&dir);
        let got = log.run_safely("deck:update", "fallback", || -> Result<&str, String> {
            panic!("index out of range")
        });
        assert_eq!(got, "fallback");
        assert!(
            fs::read_to_string(dir.join(LOG_FILENAME))
                .unwrap()
                .contains("deck:update — panic: index out of range")
        );
    }

    #[test]
    fn error_value_keeps_the_cause_chain() {
        #[derive(Debug)]
        struct Outer(std::io::Error);
        impl Display for Outer {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("could not read session")
            }
        }
        impl std::error::Error for Outer {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }
        let dir = temp_dir("runlog");
        let log = quiet(&dir);
        let err = Outer(std::io::Error::other("permission denied"));
        log.error_value("serato:read", &err, None);
        let entry: Value =
            serde_json::from_str(fs::read_to_string(dir.join(JSONL_FILENAME)).unwrap().trim())
                .unwrap();
        assert_eq!(
            entry["stack"],
            "could not read session\ncaused by: permission denied"
        );
    }

    #[test]
    fn summary_line() {
        let dir = temp_dir("runlog");
        let log = quiet(&dir);
        assert_eq!(log.summarize_run(), None);
        log.error("a", "x");
        assert!(
            log.summarize_run()
                .unwrap()
                .starts_with("1 issue this run — see ")
        );
        log.error("a", "y");
        assert!(
            log.summarize_run()
                .unwrap()
                .starts_with("2 issues this run — see ")
        );
    }

    #[test]
    fn caps_each_file_with_one_backup() {
        let dir = temp_dir("runlog");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(LOG_FILENAME), vec![b'a'; MAX_FILE_BYTES as usize]).unwrap();
        quiet(&dir).error("a", "after the cap");
        assert!(dir.join("errors.log.1").exists());
        assert!(fs::metadata(dir.join(LOG_FILENAME)).unwrap().len() < 200);
    }

    #[test]
    fn never_fails_when_it_cannot_write() {
        let dir = temp_dir("runlog");
        fs::write(&dir, b"a file, not a directory").unwrap();
        let log = quiet(&dir);
        log.error("a", "dropped");
        assert_eq!(log.issue_count(), 1);
    }

    // The only test that touches the process-wide log and hook.
    #[test]
    fn panic_hook_records_a_thread_panic_once() {
        let dir = temp_dir("runlog-global");
        let log = init(&dir);
        log.set_echo(false);
        install_panic_hook();
        let joined = std::thread::Builder::new()
            .name("serato-watch".into())
            .spawn(|| panic!("session file vanished"))
            .unwrap()
            .join();
        assert!(joined.is_err());
        let text = fs::read_to_string(log.log_path()).unwrap();
        assert_eq!(
            text.matches("ERROR panic:serato-watch — panic: session file vanished")
                .count(),
            1
        );
        let entry: Value = serde_json::from_str(
            fs::read_to_string(dir.join(JSONL_FILENAME))
                .unwrap()
                .lines()
                .last()
                .unwrap(),
        )
        .unwrap();
        assert!(entry["stack"].as_str().unwrap().contains("runlog.rs:"));
    }

    #[test]
    fn threads_never_interleave_lines() {
        let dir = temp_dir("runlog");
        let log = std::sync::Arc::new(quiet(&dir));
        let handles: Vec<_> = (0..8)
            .map(|t| {
                let log = log.clone();
                std::thread::spawn(move || {
                    for i in 0..50 {
                        log.error(&format!("t{t}"), format!("message {i} {}", "x".repeat(200)));
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let text = fs::read_to_string(dir.join(JSONL_FILENAME)).unwrap();
        assert_eq!(text.lines().count(), 400);
        assert!(
            text.lines()
                .all(|l| serde_json::from_str::<Value>(l).is_ok())
        );
    }
}
