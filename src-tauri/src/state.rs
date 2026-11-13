//! Starea aplicației, partajată între comenzi, scheduler și WhatsApp.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use chrono::{DateTime, Local};
use scadentar_core::format::Digest;
use scadentar_core::notify::NotifyState;
use scadentar_core::{Loaded, Settings, Snapshot};
use scadentar_wa::WaService;

/// Ultimul fișier Excel citit.
#[derive(Default)]
pub struct FileState {
    pub path: Option<String>,
    pub loaded: Option<Loaded>,
    /// Data modificării fișierului în momentul citirii (ca să observăm când se schimbă).
    pub mtime: Option<SystemTime>,
    pub read_at: Option<DateTime<Local>>,
    pub error: Option<String>,
}

pub struct AppState {
    pub config_dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub notify: Mutex<NotifyState>,
    pub file: Mutex<FileState>,
    pub snapshot: Mutex<Snapshot>,
    /// Ultima alertă afișată (o citește fereastra pop-up).
    pub digest: Mutex<Option<Digest>>,
    /// Ultima eroare la trimiterea pe WhatsApp (se șterge la următorul mesaj reușit).
    pub wa_error: Mutex<Option<String>>,
    pub wa: WaService,
    /// Doar o citire de fișier / verificare o dată.
    pub refreshing: tokio::sync::Mutex<()>,
}

impl AppState {
    pub fn settings_path(&self) -> PathBuf {
        self.config_dir.join("setari.json")
    }

    pub fn notify_path(&self) -> PathBuf {
        self.config_dir.join("stare.json")
    }
}
