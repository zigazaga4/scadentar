use serde::{Deserialize, Serialize};

/// Setările aplicației, salvate ca JSON. Câmpurile lipsă primesc valorile implicite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Fișierul Excel cu facturile. `None` = prima pornire, încă nu a fost ales.
    pub excel_path: Option<String>,
    /// Cu câte zile înainte de scadență începe alerta.
    pub warn_days: u32,
    /// Scadențele depășite de mai mult de atâtea zile nu mai sunt alertate (0 = fără limită).
    pub overdue_max_days: u32,
    /// Restul de plată sub această valoare e ignorat (diferențe de câțiva bani).
    pub min_rest: f64,
    /// La câte minute se reciteste fișierul și se verifică scadențele.
    pub check_minutes: u32,
    /// La câte ore se repetă alerta pentru scadențele de azi / depășite, cât timp nu sunt achitate.
    pub repeat_hours: u32,
    /// Alertele apar doar între aceste ore (ora locală, `activeFrom` inclusiv, `activeTo` exclusiv).
    pub active_from: u32,
    pub active_to: u32,
    /// Foile (furnizorii) pe care nu vrei să-i urmărești.
    pub excluded_sheets: Vec<String>,
    pub notify_toast: bool,
    pub notify_popup: bool,
    pub notify_sound: bool,
    pub notify_whatsapp: bool,
    /// Pornește singură la deschiderea Windows-ului.
    pub autostart: bool,
    /// S-a făcut deja legătura cu WhatsApp (la pornire se reconectează automat).
    pub wa_paired: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            excel_path: None,
            warn_days: 7,
            overdue_max_days: 60,
            min_rest: 1.0,
            check_minutes: 15,
            repeat_hours: 4,
            active_from: 8,
            active_to: 20,
            excluded_sheets: Vec::new(),
            notify_toast: true,
            notify_popup: true,
            notify_sound: true,
            notify_whatsapp: true,
            autostart: true,
            wa_paired: false,
        }
    }
}

impl Settings {
    /// Aduce valorile în limite rezonabile (după ce utilizatorul le-a scris de mână).
    pub fn sanitized(mut self) -> Self {
        self.warn_days = self.warn_days.clamp(0, 90);
        self.overdue_max_days = self.overdue_max_days.min(3650);
        self.min_rest = if self.min_rest.is_finite() { self.min_rest.max(0.0) } else { 1.0 };
        self.check_minutes = self.check_minutes.clamp(1, 24 * 60);
        self.repeat_hours = self.repeat_hours.clamp(1, 72);
        self.active_from = self.active_from.min(23);
        self.active_to = self.active_to.min(24);
        self
    }

    pub fn is_excluded(&self, sheet: &str) -> bool {
        self.excluded_sheets.iter().any(|s| s == sheet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"warnDays": 3}"#).unwrap();
        assert_eq!(s.warn_days, 3);
        assert_eq!(s.repeat_hours, 4);
        assert!(s.notify_whatsapp);
    }

    #[test]
    fn sanitized_clamps_wild_values() {
        let s = Settings { warn_days: 9999, check_minutes: 0, repeat_hours: 0, active_from: 99, active_to: 99, min_rest: f64::NAN, ..Default::default() }.sanitized();
        assert_eq!((s.warn_days, s.check_minutes, s.repeat_hours, s.active_from, s.active_to), (90, 1, 1, 23, 24));
        assert_eq!(s.min_rest, 1.0);
    }
}
