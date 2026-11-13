use serde::{Deserialize, Serialize};

/// Cum se hotărăște momentul unei alerte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlertMode {
    /// Imediat ce apare ceva nou, apoi reamintiri la interval (între orele active).
    #[default]
    Interval,
    /// Doar la orele fixe alese de utilizator, în fiecare zi.
    Times,
}

/// Cea mai mare oră fixă de alertă pe zi.
pub const MAX_ALERT_TIMES: usize = 8;
/// Ora fixă folosită când lista e goală.
pub const DEFAULT_ALERT_TIME: &str = "09:00";

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
    /// Se aplică doar în modul `interval`.
    pub active_from: u32,
    pub active_to: u32,
    /// `interval`: alertă imediat ce apare ceva nou și reamintiri; `times`: doar la orele din `alertTimes`.
    pub alert_mode: AlertMode,
    /// Orele fixe ale alertei, `HH:MM` (modul `times`).
    pub alert_times: Vec<String>,
    /// Contractele importante (ex. leasing-urile mașinilor), după numele foii: când o scadență a lor
    /// e depășită, alerta este serioasă.
    pub critical_sheets: Vec<String>,
    /// La câte ore se repetă alerta serioasă (modul `interval`).
    pub critical_repeat_hours: u32,
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
            alert_mode: AlertMode::Interval,
            alert_times: vec![DEFAULT_ALERT_TIME.to_string()],
            critical_sheets: Vec::new(),
            critical_repeat_hours: 2,
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
        self.critical_repeat_hours = self.critical_repeat_hours.clamp(1, 72);
        let mut minutes = self.alert_minutes();
        minutes.truncate(MAX_ALERT_TIMES);
        self.alert_times = if minutes.is_empty() { vec![DEFAULT_ALERT_TIME.to_string()] } else { minutes.into_iter().map(format_time).collect() };
        self
    }

    pub fn is_excluded(&self, sheet: &str) -> bool {
        self.excluded_sheets.iter().any(|s| s == sheet)
    }

    /// Foaia e un contract important (alertă serioasă când e depășită)?
    pub fn is_critical(&self, sheet: &str) -> bool {
        self.critical_sheets.iter().any(|s| s == sheet)
    }

    /// Orele fixe de alertă, ca minute de la miezul nopții, crescător.
    pub fn alert_minutes(&self) -> Vec<u32> {
        let mut minutes: Vec<u32> = self.alert_times.iter().filter_map(|t| parse_time(t)).collect();
        minutes.sort_unstable();
        minutes.dedup();
        minutes
    }
}

/// `"9:05"` / `"09:05"` → minute de la miezul nopții (545). `None` dacă nu e o oră validă.
pub fn parse_time(text: &str) -> Option<u32> {
    let (h, m) = text.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// 545 → `"09:05"`.
pub fn format_time(minutes: u32) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
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
        assert_eq!(s.critical_repeat_hours, 2);
    }

    #[test]
    fn alert_times_are_normalised_sorted_and_never_empty() {
        let s = Settings { alert_times: vec!["14:30".into(), "9:00".into(), "nu e oră".into(), "09:00".into(), "25:00".into()], ..Default::default() }.sanitized();
        assert_eq!(s.alert_times, ["09:00", "14:30"]);
        assert_eq!(s.alert_minutes(), [540, 870]);
        let empty = Settings { alert_times: vec![], ..Default::default() }.sanitized();
        assert_eq!(empty.alert_times, ["09:00"]);
        let many = Settings { alert_times: (0..12).map(|h| format!("{h:02}:00")).collect(), ..Default::default() }.sanitized();
        assert_eq!(many.alert_times.len(), MAX_ALERT_TIMES);
    }

    #[test]
    fn time_parsing() {
        assert_eq!(parse_time("07:05"), Some(425));
        assert_eq!(parse_time(" 7:5 "), Some(425));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("12:60"), None);
        assert_eq!(parse_time("12"), None);
        assert_eq!(format_time(425), "07:05");
    }

    #[test]
    fn critical_sheets_are_matched_by_exact_name() {
        let s = Settings { critical_sheets: vec!["Mercedes".into()], ..Default::default() };
        assert!(s.is_critical("Mercedes"));
        assert!(!s.is_critical("Mercedes Service"));
    }
}
