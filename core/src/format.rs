//! Textele afișate utilizatorului: sume, date, „în 3 zile”, rezumatul alertei și mesajul WhatsApp.

use chrono::NaiveDate;
use serde::Serialize;

use crate::model::{Alertable, Group, Urgency};
use crate::notify::group;

/// Suma cu separatorul de mii „.” și zecimale cu virgulă: `1.234,56`.
pub fn money(value: f64) -> String {
    let cents = (value.abs() * 100.0).round() as u64;
    let (whole, frac) = (cents / 100, cents % 100);
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(c);
    }
    let sign = if value < 0.0 && cents > 0 { "-" } else { "" };
    format!("{sign}{grouped},{frac:02}")
}

/// `13.11.2026`.
pub fn date_ro(d: NaiveDate) -> String {
    d.format("%d.%m.%Y").to_string()
}

/// `13.11`, sau cu anul dacă nu e anul curent.
pub fn date_short(d: NaiveDate, today: NaiveDate) -> String {
    if d.format("%Y").to_string() == today.format("%Y").to_string() {
        d.format("%d.%m").to_string()
    } else {
        date_ro(d)
    }
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Poziția față de azi: „azi”, „mâine”, „în 3 zile”, „ieri”, „depășită cu 4 zile”.
pub fn when(days: i64) -> String {
    match days {
        0 => "azi".into(),
        1 => "mâine".into(),
        -1 => "ieri".into(),
        d if d > 1 => format!("în {d} zile"),
        d => format!("depășită cu {} zile", -d),
    }
}

/// Rezumatul unei alerte, gata de afișat.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Digest {
    /// Ex.: „3 scadențe depășite · 2 azi · 5 în curând”.
    pub title: String,
    /// Textul scurt pentru notificarea Windows.
    pub toast: String,
    /// Mesajul complet pentru WhatsApp.
    pub whatsapp: String,
    pub groups: Vec<Group>,
    pub overdue: usize,
    pub today: usize,
    pub soon: usize,
    pub total_rest: f64,
    /// Când a fost generată: `13.11.2026`.
    pub date: String,
}

impl Digest {
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

/// Câte linii de furnizor intră în notificarea scurtă și în mesajul WhatsApp.
const TOAST_LINES: usize = 3;
const WHATSAPP_LINES: usize = 40;

pub fn build_digest(alerts: &[Alertable], today: NaiveDate) -> Digest {
    let count = |u: Urgency| alerts.iter().filter(|a| a.urgency == u).count();
    let (overdue, due_today, soon) = (count(Urgency::Overdue), count(Urgency::Today), count(Urgency::Soon));
    let groups = group(alerts);
    let total_rest = alerts.iter().map(|a| a.item.rest).sum();

    let mut parts = Vec::new();
    if overdue > 0 {
        parts.push(format!("{} depășite", plural(overdue, "scadență", "scadențe")));
    }
    if due_today > 0 {
        parts.push(format!("{due_today} azi"));
    }
    if soon > 0 {
        parts.push(format!("{soon} în curând"));
    }
    let title = parts.join(" · ");

    Digest {
        toast: toast_text(&groups, today),
        whatsapp: whatsapp_text(&groups, today, &title, total_rest),
        title,
        groups,
        overdue,
        today: due_today,
        soon,
        total_rest,
        date: date_ro(today),
    }
}

fn line(g: &Group, today: NaiveDate) -> String {
    let what = if g.count == 1 { String::new() } else { format!(" ({})", plural(g.count, "factură", "facturi")) };
    format!("{} — {} {} — {}{}", g.foaie, date_short(g.scadenta, today), when(g.days), money(g.total_rest), what)
}

fn toast_text(groups: &[Group], today: NaiveDate) -> String {
    let mut lines: Vec<String> = groups.iter().take(TOAST_LINES).map(|g| line(g, today)).collect();
    if groups.len() > TOAST_LINES {
        lines.push(format!("…și încă {}", plural(groups.len() - TOAST_LINES, "furnizor/dată", "furnizori/date")));
    }
    lines.join("\n")
}

fn whatsapp_text(groups: &[Group], today: NaiveDate, title: &str, total_rest: f64) -> String {
    let mut out = format!("⚠️ *Scadențe plăți* — {}\n{}\n", date_ro(today), title);
    let mut shown = 0;
    for (urgency, heading) in [(Urgency::Overdue, "🔴 *DEPĂȘITE*"), (Urgency::Today, "🟠 *AZI*"), (Urgency::Soon, "🟡 *ÎN CURÂND*")] {
        let section: Vec<&Group> = groups.iter().filter(|g| g.urgency == urgency).collect();
        if section.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{heading}\n"));
        for g in section {
            if shown == WHATSAPP_LINES {
                break;
            }
            out.push_str(&format!("• {}\n", line(g, today)));
            shown += 1;
        }
    }
    if groups.len() > shown {
        out.push_str(&format!("…și încă {}\n", plural(groups.len() - shown, "linie", "linii")));
    }
    out.push_str(&format!("\nTotal de plată: *{}*", money(total_rest)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Scadenta;
    use crate::notify::classify;
    use crate::settings::Settings;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn item(foaie: &str, factura: &str, due: NaiveDate, rest: f64) -> Scadenta {
        Scadenta { foaie: foaie.into(), factura: factura.into(), emisa: None, scadenta: due, suma: rest, rest, rand: 1 }
    }

    #[test]
    fn money_uses_romanian_separators() {
        assert_eq!(money(0.0), "0,00");
        assert_eq!(money(5.5), "5,50");
        assert_eq!(money(1234.56), "1.234,56");
        assert_eq!(money(1_234_567.891), "1.234.567,89");
        assert_eq!(money(999.999), "1.000,00");
        assert_eq!(money(-18.61), "-18,61");
        assert_eq!(money(-0.001), "0,00");
    }

    #[test]
    fn days_read_naturally() {
        assert_eq!(when(0), "azi");
        assert_eq!(when(1), "mâine");
        assert_eq!(when(5), "în 5 zile");
        assert_eq!(when(-1), "ieri");
        assert_eq!(when(-4), "depășită cu 4 zile");
    }

    #[test]
    fn digest_summarises_and_builds_the_whatsapp_message() {
        let today = d(2026, 11, 13);
        let items = vec![
            item("Furnizor Test", "A1", d(2026, 11, 9), 984.29),
            item("Furnizor Test", "A2", d(2026, 11, 9), 849.64),
            item("Renault", "R1", d(2026, 11, 15), 4094.65),
            item("BCR", "rata", today, 757.80),
        ];
        let alerts = classify(&items, &Settings::default(), today).alerts;
        let digest = build_digest(&alerts, today);
        assert_eq!(digest.title, "2 scadențe depășite · 1 azi · 1 în curând");
        assert_eq!((digest.overdue, digest.today, digest.soon), (2, 1, 1));
        assert_eq!(digest.groups.len(), 3);
        assert!(digest.whatsapp.contains("🔴 *DEPĂȘITE*"));
        assert!(digest.whatsapp.contains("• Furnizor Test — 09.11 depășită cu 4 zile — 1.833,93 (2 facturi)"));
        assert!(digest.whatsapp.contains("• Renault — 15.11 în 2 zile — 4.094,65"));
        assert!(digest.whatsapp.ends_with("Total de plată: *6.686,38*"));
        // ordinea secțiunilor: depășite, azi, în curând
        let (a, b, c) = (digest.whatsapp.find("DEPĂȘITE").unwrap(), digest.whatsapp.find("AZI*").unwrap(), digest.whatsapp.find("ÎN CURÂND").unwrap());
        assert!(a < b && b < c);
        assert_eq!(digest.toast.lines().count(), 3);
    }

    #[test]
    fn long_lists_are_cut_with_a_note() {
        let today = d(2026, 11, 13);
        let items: Vec<Scadenta> = (0..60).map(|i| item(&format!("Furnizor {i}"), "F", d(2026, 11, 14), 10.0)).collect();
        let alerts = classify(&items, &Settings::default(), today).alerts;
        let digest = build_digest(&alerts, today);
        assert!(digest.whatsapp.contains("…și încă 20 linii"));
        assert!(digest.toast.contains("…și încă 57 furnizori/date"));
    }
}
