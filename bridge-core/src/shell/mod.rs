//! What the Electron app (`bridge-app/src/main`) adds around the Bridge, ported for a native shell.
//!
//! - [`api`]: sign-in, token check, the Bridge API key, the event list and the event health check.
//! - [`logfile`]: the rotating `bridge.log`.
//! - [`netcheck`]: the warning when two network interfaces share a subnet.
//! - [`settings`]: the stored API URL, last event and Bridge settings, and the token vault.
//! - [`keychain`]: the token in the OS keychain, with a logged in-memory fallback.
//! - [`runlog`]: the SYS-06 run log (errors.jsonl + errors.log), catch and carry on.
//! - [`watch`]: the event health watcher and the start-up network warnings.
//!
//! Each module keeps the behaviour its TypeScript twin's tests pin down. Where the port is
//! stricter (a malformed server reply is an error, not `undefined`), the module docs say so.

pub mod api;
pub mod keychain;
pub mod logfile;
pub mod netcheck;
pub mod runlog;
pub mod settings;
pub mod watch;

#[cfg(test)]
pub(crate) mod mock;
