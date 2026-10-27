//! The user's update preference, shared by the CLI (its daily check) and the
//! gateway (the Settings page): `~/.meshy-dcc/settings.json`, for example
//! `{"update_check": false}`. A missing or unreadable file means the default.

use std::path::PathBuf;

use serde_json::{Map, Value};

/// Overrides the settings file (tests, unusual homes).
pub const SETTINGS_FILE_ENV: &str = "MESHY_DCC_SETTINGS_FILE";
const UPDATE_CHECK: &str = "update_check";

pub fn settings_path() -> PathBuf {
    if let Some(path) = std::env::var_os(SETTINGS_FILE_ENV).filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".meshy-dcc")
        .join("settings.json")
}

fn read() -> Map<String, Value> {
    std::fs::read(settings_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

/// Whether `meshy-dcc` checks for a new version once a day (default on).
pub fn update_check_enabled() -> bool {
    read()
        .get(UPDATE_CHECK)
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

/// Turn the daily check on or off, keeping any other settings in the file.
pub fn set_update_check(enabled: bool) -> anyhow::Result<()> {
    let path = settings_path();
    let mut settings = read();
    settings.insert(UPDATE_CHECK.into(), Value::Bool(enabled));
    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer_pretty(&mut file, &settings)?;
    file.persist(&path)?;
    Ok(())
}

/// Another boolean setting in the same file (for example `telemetry`).
pub fn get_bool(key: &str, default: bool) -> bool {
    read().get(key).and_then(Value::as_bool).unwrap_or(default)
}

/// Set another boolean setting, keeping the rest of the file.
pub fn set_bool(key: &str, value: bool) -> anyhow::Result<()> {
    let path = settings_path();
    let mut settings = read();
    settings.insert(key.into(), Value::Bool(value));
    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer_pretty(&mut file, &settings)?;
    file.persist(&path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_check_defaults_on_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        // SAFETY: the only test in this crate that touches the variable.
        unsafe { std::env::set_var(SETTINGS_FILE_ENV, &path) };
        assert!(update_check_enabled());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"other": 1}"#).unwrap();
        set_update_check(false).unwrap();
        assert!(!update_check_enabled());
        let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            saved,
            serde_json::json!({"other": 1, "update_check": false})
        );
        std::fs::write(&path, "not json").unwrap();
        assert!(update_check_enabled());
        unsafe { std::env::remove_var(SETTINGS_FILE_ENV) };
    }
}
