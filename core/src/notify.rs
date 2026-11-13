//! Ce intră în alertă și când se trimite o nouă alertă (fără să deranjăm omul la fiecare verificare).

use std::collections::{BTreeMap, HashMap};

use chrono::{NaiveDate, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};

use crate::model::{Alertable, Group, Scadenta, Urgency};
use crate::settings::Settings;

/// Cât de departe în viitor se arată scadențele „care urmează” în aplicație (nu declanșează alerte).
pub const UPCOMING_DAYS: i64 = 60;
/// Scadențele care nu intră în alertă dar sunt doar „în curând” se repetă cel mult o dată pe zi.
const SOON_REPEAT_HOURS: u32 = 24;

/// Facturile împărțite după cum apar în aplicație.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Classified {
    /// În fereastra de alertă: depășite (nu prea demult), de azi și cele din următoarele zile.
    pub alerts: Vec<Alertable>,
    /// După fereastra de alertă, până la 60 de zile: doar de informare.
    pub upcoming: Vec<Alertable>,
    /// Depășite de prea mult timp ca să mai fie alertate, pe furnizor.
    pub older: Vec<Older>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Older {
    pub foaie: String,
    pub count: usize,
    pub total_rest: f64,
    pub oldest: NaiveDate,
}

/// Împarte facturile deschise după zilele rămase până la scadență.
pub fn classify(items: &[Scadenta], settings: &Settings, today: NaiveDate) -> Classified {
    let mut out = Classified::default();
    let mut older: BTreeMap<String, Older> = BTreeMap::new();
    for item in items {
        if settings.is_excluded(&item.foaie) || item.rest < settings.min_rest {
            continue;
        }
        let days = (item.scadenta - today).num_days();
        let urgency = match days {
            d if d < 0 => Urgency::Overdue,
            0 => Urgency::Today,
            _ => Urgency::Soon,
        };
        let entry = Alertable { item: item.clone(), urgency, days, critical: settings.is_critical(&item.foaie) };
        if days > UPCOMING_DAYS {
            continue;
        } else if days > settings.warn_days as i64 {
            out.upcoming.push(entry);
        } else if settings.overdue_max_days == 0 || days >= -(settings.overdue_max_days as i64) {
            out.alerts.push(entry);
        } else {
            let o = older.entry(item.foaie.clone()).or_insert(Older { foaie: item.foaie.clone(), count: 0, total_rest: 0.0, oldest: item.scadenta });
            o.count += 1;
            o.total_rest += item.rest;
            o.oldest = o.oldest.min(item.scadenta);
        }
    }
    let by_date = |a: &Alertable, b: &Alertable| a.item.scadenta.cmp(&b.item.scadenta).then_with(|| a.item.foaie.cmp(&b.item.foaie)).then(a.item.rand.cmp(&b.item.rand));
    out.alerts.sort_by(by_date);
    out.upcoming.sort_by(by_date);
    out.older = older.into_values().collect();
    out.older.sort_by(|a, b| b.total_rest.total_cmp(&a.total_rest));
    out
}

/// Grupează facturile după furnizor și dată scadentă, în ordinea datelor.
pub fn group(alerts: &[Alertable]) -> Vec<Group> {
    let mut map: BTreeMap<(NaiveDate, String), Group> = BTreeMap::new();
    for a in alerts {
        let g = map.entry((a.item.scadenta, a.item.foaie.clone())).or_insert_with(|| Group {
            foaie: a.item.foaie.clone(),
            scadenta: a.item.scadenta,
            urgency: a.urgency,
            days: a.days,
            count: 0,
            total_rest: 0.0,
            facturi: Vec::new(),
            critical: a.critical,
        });
        g.count += 1;
        g.total_rest += a.item.rest;
        if !a.item.factura.is_empty() {
            g.facturi.push(a.item.factura.clone());
        }
    }
    map.into_values().collect()
}

/// Ce s-a spus deja utilizatorului, ca să nu fie repetat la fiecare verificare.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotifyState {
    pub notified: HashMap<String, Seen>,
    /// Până la ce moment (secunde Unix) alertele sunt amânate de utilizator.
    pub snoozed_until: i64,
    /// Modul `times`: ultima oră fixă de alertă deja tratată, ca marcaj de timp al orei locale
    /// (vezi [`slot_stamp`]). 0 = niciuna încă.
    pub last_slot: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Seen {
    /// Când a fost alertat ultima dată (secunde Unix).
    pub at: i64,
    /// Cât de urgentă era atunci (`Urgency::level`).
    pub level: u8,
}

/// Identifică o factură de la o verificare la alta (fără numărul rândului, ca inserarea de rânduri
/// în Excel să nu declanșeze din nou toate alertele).
pub fn item_key(item: &Scadenta) -> String {
    format!("{}|{}|{}|{:.2}", item.foaie, item.factura, item.scadenta, item.rest)
}

/// Este `hour` (0–23) între `from` (inclusiv) și `to` (exclusiv)? Intervalul poate trece de miezul nopții.
pub fn in_active_hours(hour: u32, from: u32, to: u32) -> bool {
    match from.cmp(&to) {
        std::cmp::Ordering::Equal => true,
        std::cmp::Ordering::Less => hour >= from && hour < to,
        std::cmp::Ordering::Greater => hour >= from || hour < to,
    }
}

/// Trebuie trimisă o alertă acum? Da, dacă e în orele active, nu e amânată și cel puțin o factură
/// e nouă, a devenit mai urgentă sau i-a venit vremea să fie reamintită.
pub fn should_notify(alerts: &[Alertable], state: &NotifyState, now: i64, hour: u32, settings: &Settings) -> bool {
    if alerts.is_empty() || now < state.snoozed_until || !in_active_hours(hour, settings.active_from, settings.active_to) {
        return false;
    }
    alerts.iter().any(|a| match state.notified.get(&item_key(&a.item)) {
        None => true,
        Some(seen) if seen.level < a.urgency.level() => true,
        Some(seen) => {
            let hours = if a.is_serious() {
                settings.critical_repeat_hours
            } else if a.urgency == Urgency::Soon {
                SOON_REPEAT_HOURS
            } else {
                settings.repeat_hours
            };
            now - seen.at >= i64::from(hours) * 3600
        }
    })
}

/// Marcaj de timp pentru o dată și oră locală (ora de pe ceas, fără fus orar): doar pentru comparații.
pub fn slot_stamp(local: NaiveDateTime) -> i64 {
    local.and_utc().timestamp()
}

/// Modul `times`: a venit vremea unei ore fixe de alertă netratate încă? Întoarce marcajul orei
/// (de salvat în [`NotifyState::last_slot`] după ce a fost tratată). Contează doar orele de azi; dacă
/// calculatorul a fost oprit la ora respectivă, alerta vine la prima verificare de după.
pub fn due_slot(times: &[u32], last_slot: i64, now: NaiveDateTime) -> Option<i64> {
    let minute_now = now.hour() * 60 + now.minute();
    let latest = times.iter().copied().filter(|&m| m <= minute_now).max()?;
    let slot = now.date().and_hms_opt(latest / 60, latest % 60, 0)?;
    let stamp = slot_stamp(slot);
    (stamp > last_slot).then_some(stamp)
}

/// Notează că toate facturile din `alerts` au fost comunicate acum; le uită pe cele care nu mai sunt de actualitate.
pub fn mark_notified(alerts: &[Alertable], state: &mut NotifyState, now: i64) {
    state.notified.clear();
    for a in alerts {
        state.notified.insert(item_key(&a.item), Seen { at: now, level: a.urgency.level() });
    }
}

/// Amână alertele cu `minutes` minute.
pub fn snooze(state: &mut NotifyState, now: i64, minutes: i64) {
    state.snoozed_until = now + minutes * 60;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn item(foaie: &str, factura: &str, due: NaiveDate, rest: f64) -> Scadenta {
        Scadenta { foaie: foaie.into(), factura: factura.into(), emisa: None, scadenta: due, suma: rest, rest, rand: 7 }
    }

    const TODAY: (i32, u32, u32) = (2026, 11, 13);
    fn today() -> NaiveDate {
        d(TODAY.0, TODAY.1, TODAY.2)
    }

    #[test]
    fn items_are_split_by_window() {
        let items = vec![
            item("A", "overdue-recent", d(2026, 11, 9), 100.0),
            item("A", "today", today(), 100.0),
            item("A", "soon", d(2026, 11, 20), 100.0),
            item("A", "edge-soon", d(2026, 11, 20), 100.0), // exact 7 zile: încă în alertă
            item("A", "upcoming", d(2026, 12, 20), 100.0),
            item("A", "far", d(2027, 6, 1), 100.0),
            item("B", "stale", d(2024, 1, 10), 500.0),
            item("B", "stale2", d(2024, 3, 10), 250.0),
            item("C", "pennies", today(), 0.5),
        ];
        let c = classify(&items, &Settings::default(), today());
        let names = |v: &[Alertable]| v.iter().map(|a| a.item.factura.clone()).collect::<Vec<_>>();
        assert_eq!(names(&c.alerts), ["overdue-recent", "today", "soon", "edge-soon"]);
        assert_eq!(names(&c.upcoming), ["upcoming"]);
        assert_eq!(c.older.len(), 1);
        assert_eq!((c.older[0].count, c.older[0].total_rest, c.older[0].oldest), (2, 750.0, d(2024, 1, 10)));
        assert_eq!(c.alerts[0].urgency, Urgency::Overdue);
        assert_eq!(c.alerts[0].days, -4);
        assert_eq!(c.alerts[1].urgency, Urgency::Today);
        assert_eq!(c.alerts[2].urgency, Urgency::Soon);
    }

    #[test]
    fn excluded_sheets_and_unlimited_overdue() {
        let items = vec![item("A", "x", d(2020, 1, 1), 100.0), item("B", "y", d(2026, 11, 14), 100.0)];
        let settings = Settings { excluded_sheets: vec!["B".into()], overdue_max_days: 0, ..Default::default() };
        let c = classify(&items, &settings, today());
        assert_eq!(c.alerts.len(), 1);
        assert_eq!(c.alerts[0].item.foaie, "A");
        assert!(c.older.is_empty());
    }

    #[test]
    fn groups_merge_same_supplier_and_date() {
        let items = vec![
            item("Leasing Test", "MBL1", d(2026, 11, 20), 700.0),
            item("Leasing Test", "MBL2", d(2026, 11, 20), 800.0),
            item("Leasing Test", "MBL3", d(2026, 11, 19), 50.0),
            item("BCR", "rata", d(2026, 11, 18), 757.8),
        ];
        let c = classify(&items, &Settings::default(), today());
        let groups = group(&c.alerts);
        assert_eq!(groups.len(), 3);
        assert_eq!((groups[0].foaie.as_str(), groups[0].count), ("BCR", 1));
        assert_eq!((groups[1].foaie.as_str(), groups[1].count), ("Leasing Test", 1));
        assert_eq!((groups[2].foaie.as_str(), groups[2].count, groups[2].total_rest), ("Leasing Test", 2, 1500.0));
        assert_eq!(groups[2].facturi, ["MBL1", "MBL2"]);
    }

    #[test]
    fn active_hours_can_wrap_midnight() {
        assert!(in_active_hours(8, 8, 20));
        assert!(!in_active_hours(20, 8, 20));
        assert!(!in_active_hours(7, 8, 20));
        assert!(in_active_hours(23, 22, 6));
        assert!(in_active_hours(3, 22, 6));
        assert!(!in_active_hours(12, 22, 6));
        assert!(in_active_hours(12, 0, 0));
    }

    #[test]
    fn notifies_once_then_repeats_after_the_interval() {
        let settings = Settings::default(); // repetare 4 ore, ore active 8–20
        let items = vec![item("A", "x", d(2026, 11, 12), 100.0)];
        let alerts = classify(&items, &settings, today()).alerts;
        let mut state = NotifyState::default();
        let t0 = 1_000_000;

        assert!(should_notify(&alerts, &state, t0, 9, &settings), "prima dată");
        mark_notified(&alerts, &mut state, t0);
        assert!(!should_notify(&alerts, &state, t0 + 60, 9, &settings), "imediat după");
        assert!(!should_notify(&alerts, &state, t0 + 3 * 3600, 12, &settings), "înainte de 4 ore");
        assert!(should_notify(&alerts, &state, t0 + 4 * 3600, 13, &settings), "după 4 ore");
    }

    #[test]
    fn nothing_before_opening_hours_or_while_snoozed_or_when_empty() {
        let settings = Settings::default();
        let alerts = classify(&[item("A", "x", today(), 100.0)], &settings, today()).alerts;
        let mut state = NotifyState::default();
        assert!(!should_notify(&alerts, &state, 5, 7, &settings), "prea devreme");
        assert!(!should_notify(&alerts, &state, 5, 21, &settings), "prea târziu");
        assert!(should_notify(&alerts, &state, 5, 9, &settings));
        snooze(&mut state, 5, 60);
        assert!(!should_notify(&alerts, &state, 5 + 59 * 60, 9, &settings), "amânat");
        assert!(should_notify(&alerts, &state, 5 + 61 * 60, 9, &settings), "amânarea a trecut");
        assert!(!should_notify(&[], &NotifyState::default(), 5, 9, &settings), "nimic de alertat");
    }

    #[test]
    fn escalation_and_new_items_alert_immediately() {
        let settings = Settings::default();
        let soon = classify(&[item("A", "x", d(2026, 11, 15), 100.0)], &settings, today()).alerts;
        let mut state = NotifyState::default();
        mark_notified(&soon, &mut state, 1000);
        assert!(!should_notify(&soon, &state, 2000, 9, &settings));

        // aceeași factură, a doua zi e „azi” → mai urgent
        let next_day = d(2026, 11, 15);
        let escalated = classify(&[item("A", "x", d(2026, 11, 15), 100.0)], &settings, next_day).alerts;
        assert_eq!(escalated[0].urgency, Urgency::Today);
        assert!(should_notify(&escalated, &state, 2000, 9, &settings));

        // factură nouă
        let mut both = soon.clone();
        both.extend(classify(&[item("B", "y", d(2026, 11, 16), 5.0 + 100.0)], &settings, today()).alerts);
        assert!(should_notify(&both, &state, 2000, 9, &settings));
    }

    #[test]
    fn soon_items_repeat_only_daily() {
        let settings = Settings::default();
        let alerts = classify(&[item("A", "x", d(2026, 11, 19), 100.0)], &settings, today()).alerts;
        let mut state = NotifyState::default();
        mark_notified(&alerts, &mut state, 0);
        assert!(!should_notify(&alerts, &state, 23 * 3600, 12, &settings));
        assert!(should_notify(&alerts, &state, 24 * 3600, 12, &settings));
    }

    #[test]
    fn paid_items_are_forgotten_so_a_new_amount_alerts_again() {
        let settings = Settings::default();
        let alerts = classify(&[item("A", "x", today(), 100.0)], &settings, today()).alerts;
        let mut state = NotifyState::default();
        mark_notified(&alerts, &mut state, 0);
        mark_notified(&[], &mut state, 10);
        assert!(state.notified.is_empty());
        // rest diferit (plată parțială) = factură „nouă”
        let partial = classify(&[item("A", "x", today(), 40.0)], &settings, today()).alerts;
        assert!(should_notify(&partial, &state, 20, 9, &settings));
    }

    #[test]
    fn critical_sheets_flag_their_items_and_overdue_ones_are_serious() {
        let settings = Settings { critical_sheets: vec!["Leasing".into()], ..Default::default() };
        let items = vec![
            item("Leasing", "rata-veche", d(2026, 11, 10), 700.0),
            item("Leasing", "rata-azi", today(), 700.0),
            item("Service", "factura-veche", d(2026, 11, 10), 90.0),
        ];
        let alerts = classify(&items, &settings, today()).alerts;
        let serious: Vec<_> = alerts.iter().filter(|a| a.is_serious()).map(|a| a.item.factura.as_str()).collect();
        assert_eq!(serious, ["rata-veche"]);
        assert!(alerts.iter().any(|a| a.critical && a.urgency == Urgency::Today && !a.is_serious()));
        let groups = group(&alerts);
        assert!(groups.iter().any(|g| g.is_serious() && g.foaie == "Leasing"));
        assert!(!groups.iter().any(|g| g.is_serious() && g.foaie == "Service"));
    }

    #[test]
    fn serious_alerts_repeat_faster_than_normal_ones() {
        let settings = Settings { critical_sheets: vec!["Leasing".into()], ..Default::default() }; // normal 4 h, serios 2 h
        let serious = classify(&[item("Leasing", "r", d(2026, 11, 10), 700.0)], &settings, today()).alerts;
        let normal = classify(&[item("Service", "f", d(2026, 11, 10), 700.0)], &settings, today()).alerts;
        let (mut s_state, mut n_state) = (NotifyState::default(), NotifyState::default());
        mark_notified(&serious, &mut s_state, 0);
        mark_notified(&normal, &mut n_state, 0);
        assert!(!should_notify(&serious, &s_state, 2 * 3600 - 1, 12, &settings));
        assert!(should_notify(&serious, &s_state, 2 * 3600, 12, &settings), "serios: după 2 ore");
        assert!(!should_notify(&normal, &n_state, 2 * 3600, 12, &settings), "normal: încă nu");
        assert!(should_notify(&normal, &n_state, 4 * 3600, 12, &settings));
    }

    fn at(h: u32, m: u32) -> NaiveDateTime {
        today().and_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn fixed_times_fire_once_per_slot() {
        let times = [9 * 60, 15 * 60];
        assert_eq!(due_slot(&times, 0, at(8, 59)), None, "înainte de prima oră");
        let nine = due_slot(&times, 0, at(9, 0)).expect("la 09:00");
        assert_eq!(due_slot(&times, nine, at(9, 30)), None, "ora a fost deja tratată");
        assert_eq!(due_slot(&times, nine, at(14, 59)), None);
        let three = due_slot(&times, nine, at(15, 0)).expect("la 15:00");
        assert!(three > nine);
        assert_eq!(due_slot(&times, three, at(23, 59)), None, "nimic până a doua zi");
    }

    #[test]
    fn a_missed_slot_is_caught_up_once_when_the_app_starts_late() {
        let times = [9 * 60, 15 * 60];
        // calculatorul pornește la 17:00; ambele ore au trecut, dar se face o singură alertă (cea mai recentă)
        let slot = due_slot(&times, 0, at(17, 0)).expect("recuperare");
        assert_eq!(slot, slot_stamp(at(15, 0)));
        assert_eq!(due_slot(&times, slot, at(17, 1)), None);
    }

    #[test]
    fn changing_the_schedule_resets_the_bookkeeping() {
        // în aplicație, la schimbarea orelor `last_slot` devine „acum”: orele de până atunci nu se mai alertează
        let times = [9 * 60];
        let now = at(11, 0);
        assert_eq!(due_slot(&times, slot_stamp(now), now), None);
        assert!(due_slot(&times, slot_stamp(now), today().succ_opt().unwrap().and_hms_opt(9, 0, 0).unwrap()).is_some(), "a doua zi la 09:00");
    }
}
