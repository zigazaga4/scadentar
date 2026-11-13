//! Tot ce are nevoie interfața ca să arate starea curentă, într-o singură structură.

use chrono::NaiveDate;
use serde::Serialize;

use crate::excel::{Loaded, SheetInfo};
use crate::format::{build_digest, Digest};
use crate::model::Alertable;
use crate::notify::{classify, Older};
use crate::settings::Settings;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Fișierul Excel urmărit (`None` = încă nu a fost ales).
    pub file: Option<String>,
    /// Când a fost citit ultima dată, ex. `13.11.2026 09:41`.
    pub loaded_at: Option<String>,
    /// Ultima eroare de citire (fișier lipsă, blocat sau invalid). Datele vechi rămân afișate.
    pub error: Option<String>,
    pub today: NaiveDate,
    /// Scadențe în fereastra de alertă.
    pub alerts: Vec<Alertable>,
    /// Scadențe care urmează (după fereastra de alertă, până la 60 de zile).
    pub upcoming: Vec<Alertable>,
    /// Depășite de prea mult timp ca să mai fie alertate.
    pub older: Vec<Older>,
    pub sheets: Vec<SheetInfo>,
    /// Facturi cu rest de plată dar fără dată scadentă.
    pub without_due_date: usize,
    pub digest: Digest,
}

impl Snapshot {
    pub fn new(file: Option<String>, loaded_at: Option<String>, error: Option<String>, loaded: Option<&Loaded>, settings: &Settings, today: NaiveDate) -> Self {
        let classified = loaded.map(|l| classify(&l.items, settings, today)).unwrap_or_default();
        let digest = build_digest(&classified.alerts, today);
        Self {
            file,
            loaded_at,
            error,
            today,
            alerts: classified.alerts,
            upcoming: classified.upcoming,
            older: classified.older,
            sheets: loaded.map(|l| l.sheets.clone()).unwrap_or_default(),
            without_due_date: loaded.map_or(0, |l| l.without_due_date),
            digest,
        }
    }
}
