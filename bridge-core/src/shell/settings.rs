//! What the Electron app stores (`store.ts`): the API URL, the last event and the Bridge
//! settings, as `config.json`, plus the sign-in token.
//!
//! Same keys and defaults as electron-store, so the file reads the same. Over the TypeScript:
//! a missing or corrupt file gives the defaults instead of an error, missing settings fields
//! take their defaults, a bad update is refused instead of stored, unknown keys (such as the
//! Electron app's `encryptedToken`) are kept, and saves are atomic (write, then rename).
//!
//! The token goes in a [`TokenVault`]. [`MemoryVault`] is the TypeScript's fallback when the OS
//! keychain is unavailable (lost on restart). The OS-keychain vault comes with the native shell.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const CONFIG_FILENAME: &str = "config.json";
pub const DEFAULT_API_URL: &str = "https://api.wrzdj.com";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BridgeSettings {
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_config: Option<Map<String, Value>>,
    pub live_threshold_seconds: f64,
    pub pause_grace_seconds: f64,
    pub now_playing_pause_seconds: f64,
    pub use_fader_detection: bool,
    pub master_deck_priority: bool,
    pub min_play_seconds: f64,
}

impl Default for BridgeSettings {
    /// `DEFAULT_SETTINGS` in `shared/types.ts`.
    fn default() -> Self {
        BridgeSettings {
            protocol: "stagelinq".into(),
            plugin_config: None,
            live_threshold_seconds: 15.0,
            pause_grace_seconds: 3.0,
            now_playing_pause_seconds: 10.0,
            use_fader_detection: false,
            master_deck_priority: false,
            min_play_seconds: 5.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Data {
    api_url: String,
    last_event_code: String,
    settings: BridgeSettings,
    #[serde(flatten)]
    other: Map<String, Value>,
}

impl Default for Data {
    fn default() -> Self {
        Data {
            api_url: DEFAULT_API_URL.into(),
            last_event_code: String::new(),
            settings: BridgeSettings::default(),
            other: Map::new(),
        }
    }
}

#[derive(Debug)]
pub struct SettingsStore {
    path: PathBuf,
    data: Data,
}

impl SettingsStore {
    /// Open `config.json` in `dir`; missing or unreadable means the defaults.
    pub fn open(dir: impl AsRef<Path>) -> Self {
        let path = dir.as_ref().join(CONFIG_FILENAME);
        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        SettingsStore { path, data }
    }

    pub fn api_url(&self) -> &str {
        &self.data.api_url
    }

    pub fn set_api_url(&mut self, url: &str) -> std::io::Result<()> {
        self.data.api_url = url.to_string();
        self.save()
    }

    pub fn last_event_code(&self) -> &str {
        &self.data.last_event_code
    }

    pub fn set_last_event_code(&mut self, code: &str) -> std::io::Result<()> {
        self.data.last_event_code = code.to_string();
        self.save()
    }

    pub fn settings(&self) -> &BridgeSettings {
        &self.data.settings
    }

    /// Merge `partial` (camelCase keys, as the UI sends) into the settings and save. A field of
    /// the wrong type refuses the whole update and changes nothing.
    pub fn update_settings(
        &mut self,
        partial: &Map<String, Value>,
    ) -> Result<BridgeSettings, String> {
        let mut merged = match serde_json::to_value(&self.data.settings) {
            Ok(Value::Object(m)) => m,
            _ => return Err("settings could not be read".into()),
        };
        for (k, v) in partial {
            merged.insert(k.clone(), v.clone());
        }
        let updated: BridgeSettings = serde_json::from_value(Value::Object(merged))
            .map_err(|e| format!("invalid settings: {e}"))?;
        let previous = std::mem::replace(&mut self.data.settings, updated.clone());
        if let Err(e) = self.save() {
            self.data.settings = previous;
            return Err(format!("settings not saved: {e}"));
        }
        Ok(updated)
    }

    fn save(&self) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(&self.data).map_err(std::io::Error::other)?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &self.path)
    }
}

/// Where the sign-in token lives.
pub trait TokenVault {
    fn get(&self) -> Option<String>;
    fn set(&mut self, token: &str);
    fn clear(&mut self);
}

/// In memory only: the DJ signs in again after a restart.
#[derive(Debug, Default)]
pub struct MemoryVault(Option<String>);

impl TokenVault for MemoryVault {
    fn get(&self) -> Option<String> {
        self.0.clone()
    }
    fn set(&mut self, token: &str) {
        self.0 = Some(token.to_string());
    }
    fn clear(&mut self) {
        self.0 = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::logfile::temp_dir;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn defaults_when_there_is_no_file() {
        let s = SettingsStore::open(temp_dir("cfg"));
        assert_eq!(s.api_url(), "https://api.wrzdj.com");
        assert_eq!(s.last_event_code(), "");
        assert_eq!(s.settings(), &BridgeSettings::default());
        assert_eq!(s.settings().protocol, "stagelinq");
    }

    #[test]
    fn values_survive_a_reopen() {
        let dir = temp_dir("cfg");
        let mut s = SettingsStore::open(&dir);
        s.set_api_url("http://192.168.1.5:8000").unwrap();
        s.set_last_event_code("ZDECUW").unwrap();
        let again = SettingsStore::open(&dir);
        assert_eq!(again.api_url(), "http://192.168.1.5:8000");
        assert_eq!(again.last_event_code(), "ZDECUW");
    }

    #[test]
    fn update_merges_a_partial_and_persists() {
        let dir = temp_dir("cfg");
        let mut s = SettingsStore::open(&dir);
        let got = s
            .update_settings(&obj(json!({"protocol": "serato", "minPlaySeconds": 8})))
            .unwrap();
        assert_eq!(got.protocol, "serato");
        assert_eq!(got.min_play_seconds, 8.0);
        assert_eq!(
            got.live_threshold_seconds, 15.0,
            "untouched fields keep their values"
        );
        assert_eq!(SettingsStore::open(&dir).settings(), &got);
    }

    #[test]
    fn a_wrongly_typed_update_changes_nothing() {
        let dir = temp_dir("cfg");
        let mut s = SettingsStore::open(&dir);
        assert!(
            s.update_settings(&obj(json!({"useFaderDetection": "yes"})))
                .is_err()
        );
        assert_eq!(s.settings(), &BridgeSettings::default());
    }

    #[test]
    fn a_corrupt_file_gives_the_defaults() {
        let dir = temp_dir("cfg");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(CONFIG_FILENAME), "{ not json").unwrap();
        assert_eq!(SettingsStore::open(&dir).api_url(), DEFAULT_API_URL);
    }

    #[test]
    fn reads_an_electron_store_file_and_keeps_its_other_keys() {
        let dir = temp_dir("cfg");
        fs::create_dir_all(&dir).unwrap();
        let electron = json!({
            "apiUrl": "https://api.example.test",
            "encryptedToken": "djEwAAAA",
            "lastEventCode": "ABC123",
            "settings": {"protocol": "traktor-broadcast", "useFaderDetection": true,
                         "pluginConfig": {"port": 8123}}
        });
        fs::write(dir.join(CONFIG_FILENAME), electron.to_string()).unwrap();
        let mut s = SettingsStore::open(&dir);
        assert_eq!(s.settings().protocol, "traktor-broadcast");
        assert!(s.settings().use_fader_detection);
        assert_eq!(
            s.settings().min_play_seconds,
            5.0,
            "missing fields take defaults"
        );
        assert_eq!(s.settings().plugin_config.as_ref().unwrap()["port"], 8123);
        s.set_last_event_code("XYZ").unwrap();
        let saved: Value =
            serde_json::from_str(&fs::read_to_string(dir.join(CONFIG_FILENAME)).unwrap()).unwrap();
        assert_eq!(saved["encryptedToken"], "djEwAAAA");
        assert_eq!(saved["lastEventCode"], "XYZ");
        assert!(!dir.join("config.json.tmp").exists());
    }

    #[test]
    fn memory_vault() {
        let mut v = MemoryVault::default();
        assert_eq!(v.get(), None);
        v.set("jwt");
        assert_eq!(v.get().as_deref(), Some("jwt"));
        v.clear();
        assert_eq!(v.get(), None);
    }
}
