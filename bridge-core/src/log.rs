//! Thin helpers over the process's run log (`shell::runlog`), for the Bridge's own modules.
//!
//! They write to the global run log when the program set one up (`runlog::init`), and do
//! nothing otherwise, so library code and tests run the same with or without it. Contexts are
//! `<area>:<detail>`, e.g. `client:post`, `serato:read`, `traktor:accept`.

use std::fmt::Display;

use crate::shell::runlog;

pub fn warn(context: &str, message: impl Display) {
    if let Some(log) = runlog::global() {
        log.warn(context, message);
    }
}

pub fn error(context: &str, message: impl Display) {
    if let Some(log) = runlog::global() {
        log.error(context, message);
    }
}

/// Run `f`; a panic is recorded under `context` and `fallback` returned, so a thread's loop
/// carries on. Without a run log, `f` just runs.
pub fn safely<T>(context: &str, fallback: T, f: impl FnOnce() -> T) -> T {
    match runlog::global() {
        Some(log) => log.run_safely(context, fallback, || Ok::<T, String>(f())),
        None => f(),
    }
}
