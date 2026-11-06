//! The sign-in token in the OS keychain, as Electron's `safeStorage` keeps it (`store.ts`):
//! Windows Credential Manager, the macOS Keychain, the Linux kernel keyring.
//!
//! When the keychain can't be used, the token is kept in memory for this run only (the DJ signs
//! in again after a restart) and the run log says so, as `store.ts` warns "token stored in
//! memory only". Every keychain failure is recorded under `shell:keychain`; none is fatal.

use super::runlog::RunLog;
use super::settings::{MemoryVault, TokenVault};

pub const SERVICE: &str = "WrzDJ Bridge";
pub const ACCOUNT: &str = "session-token";

/// Where a secret is kept. `Ok(None)` means nothing stored.
pub trait SecretStore: Send {
    fn get(&self) -> Result<Option<String>, String>;
    fn set(&self, secret: &str) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

pub struct OsKeychain {
    entry: keyring::Entry,
}

impl OsKeychain {
    pub fn new(service: &str, account: &str) -> Result<Self, String> {
        keyring::Entry::new(service, account)
            .map(|entry| OsKeychain { entry })
            .map_err(|e| e.to_string())
    }
}

impl SecretStore for OsKeychain {
    fn get(&self) -> Result<Option<String>, String> {
        match self.entry.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    fn set(&self, secret: &str) -> Result<(), String> {
        self.entry.set_password(secret).map_err(|e| e.to_string())
    }
    fn delete(&self) -> Result<(), String> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

pub struct KeychainVault {
    store: Option<Box<dyn SecretStore>>,
    memory: MemoryVault,
    log: Option<&'static RunLog>,
}

impl KeychainVault {
    /// The OS keychain, or memory only (recorded) when it can't be opened.
    pub fn os(log: Option<&'static RunLog>) -> Self {
        match OsKeychain::new(SERVICE, ACCOUNT) {
            Ok(k) => Self::with_store(Box::new(k), log),
            Err(e) => {
                let mut vault = Self::with_store_opt(None, log);
                vault.warn(&format!(
                    "OS keychain unavailable ({e}) — token will be kept in memory only"
                ));
                vault
            }
        }
    }

    pub fn with_store(store: Box<dyn SecretStore>, log: Option<&'static RunLog>) -> Self {
        Self::with_store_opt(Some(store), log)
    }

    fn with_store_opt(store: Option<Box<dyn SecretStore>>, log: Option<&'static RunLog>) -> Self {
        KeychainVault {
            store,
            memory: MemoryVault::default(),
            log,
        }
    }

    /// True once the token lives in memory only.
    pub fn memory_only(&self) -> bool {
        self.store.is_none()
    }

    fn warn(&mut self, message: &str) {
        match self.log {
            Some(log) => {
                log.warn("shell:keychain", message);
            }
            None => eprintln!("[keychain] {message}"),
        }
    }
}

impl TokenVault for KeychainVault {
    fn get(&self) -> Option<String> {
        let Some(store) = &self.store else {
            return self.memory.get();
        };
        match store.get() {
            Ok(token) => token,
            Err(e) => {
                if let Some(log) = self.log {
                    log.warn(
                        "shell:keychain",
                        format!("token could not be read ({e}) — sign in again"),
                    );
                }
                None
            }
        }
    }

    fn set(&mut self, token: &str) {
        let Some(store) = &self.store else {
            self.memory.set(token);
            return;
        };
        if let Err(e) = store.set(token) {
            self.store = None;
            self.memory.set(token);
            self.warn(&format!(
                "OS keychain refused the token ({e}) — stored in memory only for this run"
            ));
        }
    }

    fn clear(&mut self) {
        self.memory.clear();
        if let Some(store) = &self.store
            && let Err(e) = store.delete()
        {
            let message = format!("token could not be removed from the keychain ({e})");
            self.warn(&message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::logfile::temp_dir;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Fake {
        value: Mutex<Option<String>>,
        fail_get: bool,
        fail_set: bool,
        fail_delete: bool,
    }

    struct FakeStore(Arc<Fake>);

    impl SecretStore for FakeStore {
        fn get(&self) -> Result<Option<String>, String> {
            if self.0.fail_get {
                Err("locked".into())
            } else {
                Ok(self.0.value.lock().unwrap().clone())
            }
        }
        fn set(&self, s: &str) -> Result<(), String> {
            if self.0.fail_set {
                return Err("access denied".into());
            }
            *self.0.value.lock().unwrap() = Some(s.into());
            Ok(())
        }
        fn delete(&self) -> Result<(), String> {
            if self.0.fail_delete {
                return Err("busy".into());
            }
            *self.0.value.lock().unwrap() = None;
            Ok(())
        }
    }

    fn leaked_log() -> &'static RunLog {
        let dir = temp_dir("keychain");
        let log = Box::leak(Box::new(RunLog::with_paths(
            dir.join("errors.log"),
            dir.join("errors.jsonl"),
        )));
        log.set_echo(false);
        log
    }

    fn vault(fake: &Arc<Fake>, log: &'static RunLog) -> KeychainVault {
        KeychainVault::with_store(Box::new(FakeStore(fake.clone())), Some(log))
    }

    #[test]
    fn stores_reads_and_clears_through_the_keychain() {
        let fake = Arc::new(Fake::default());
        let log = leaked_log();
        let mut v = vault(&fake, log);
        assert_eq!(v.get(), None);
        v.set("jwt-1");
        assert_eq!(fake.value.lock().unwrap().as_deref(), Some("jwt-1"));
        assert_eq!(v.get().as_deref(), Some("jwt-1"));
        v.clear();
        assert_eq!(v.get(), None);
        assert_eq!(log.issue_count(), 0);
    }

    #[test]
    fn a_refused_write_falls_back_to_memory_and_is_logged() {
        let fake = Arc::new(Fake {
            fail_set: true,
            ..Default::default()
        });
        let log = leaked_log();
        let mut v = vault(&fake, log);
        v.set("jwt-2");
        assert!(v.memory_only());
        assert_eq!(v.get().as_deref(), Some("jwt-2"));
        assert_eq!(log.issue_count(), 1);
        let text = std::fs::read_to_string(log.log_path()).unwrap();
        assert!(
            text.contains("WARN shell:keychain — OS keychain refused the token (access denied)"),
            "{text}"
        );
    }

    #[test]
    fn an_unreadable_token_means_sign_in_again_and_is_logged() {
        let fake = Arc::new(Fake {
            fail_get: true,
            ..Default::default()
        });
        let log = leaked_log();
        let v = vault(&fake, log);
        assert_eq!(v.get(), None);
        assert_eq!(log.issue_count(), 1);
    }

    #[test]
    fn a_failed_delete_still_clears_memory_and_is_logged() {
        let fake = Arc::new(Fake {
            fail_delete: true,
            ..Default::default()
        });
        let log = leaked_log();
        let mut v = vault(&fake, log);
        v.set("jwt-3");
        v.clear();
        assert_eq!(log.issue_count(), 1);
    }

    /// The real keychain. Run on Windows/macOS: `cargo test -- --ignored os_keychain`.
    #[test]
    #[ignore]
    fn os_keychain_round_trip() {
        let k = OsKeychain::new("WrzDJ Bridge (test)", "round-trip").unwrap();
        k.set("secret-value").unwrap();
        assert_eq!(k.get().unwrap().as_deref(), Some("secret-value"));
        k.delete().unwrap();
        assert_eq!(k.get().unwrap(), None);
    }
}
