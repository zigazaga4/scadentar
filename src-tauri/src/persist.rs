//! Citire și scriere de fișiere JSON (setări, starea alertelor).

use std::path::Path;

use serde::{de::DeserializeOwned, Serialize};

/// Citește un fișier JSON; dacă lipsește sau e stricat întoarce valoarea implicită.
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

/// Scrie JSON într-un fișier temporar și îl mută peste cel vechi, ca o oprire în timpul scrierii să nu-l strice.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}
