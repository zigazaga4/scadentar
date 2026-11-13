//! Inima aplicației: recitește Excel-ul când s-a schimbat, calculează scadențele și hotărăște când se dă alarma.

use std::path::Path;
use std::time::Duration;

use chrono::{Local, Timelike};
use scadentar_core::notify::{due_slot, mark_notified, should_notify};
use scadentar_core::settings::AlertMode;
use scadentar_core::{read_workbook, Loaded, Settings, Snapshot};
use tauri::{AppHandle, Emitter, Manager};

use crate::alert::{self, Channels};
use crate::persist;
use crate::state::AppState;
use crate::tray;

/// La câte secunde se verifică dacă fișierul s-a schimbat și dacă a venit vremea unei alerte.
const TICK_SECONDS: u64 = 30;

/// Pornește bucla care rulează cât timp aplicația e deschisă.
pub fn spawn_scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tick(&app).await;
            tokio::time::sleep(Duration::from_secs(TICK_SECONDS)).await;
        }
    });
}

/// O verificare: citește fișierul dacă e nevoie, apoi alertează dacă e cazul.
pub async fn tick(app: &AppHandle) {
    refresh(app, false).await;
    maybe_alert(app).await;
}

/// Reface starea afișată. Fișierul se citește doar dacă `force`, dacă s-a modificat pe disc
/// sau dacă ultima citire e mai veche decât intervalul de verificare.
pub async fn refresh(app: &AppHandle, force: bool) -> Snapshot {
    let state = app.state::<AppState>();
    let _one_at_a_time = state.refreshing.lock().await;
    let settings = state.settings.lock().unwrap().clone();

    match settings.excel_path.clone() {
        None => *state.file.lock().unwrap() = Default::default(),
        Some(path) if force || needs_read(app, &path, &settings) => {
            let to_read = path.clone();
            let result = tauri::async_runtime::spawn_blocking(move || read_file(&to_read)).await;
            let mut file = state.file.lock().unwrap();
            if file.path.as_deref() != Some(path.as_str()) {
                *file = Default::default(); // alt fișier: nu păstrăm datele celui vechi
                file.path = Some(path);
            }
            match result {
                Ok(Ok((loaded, mtime))) => {
                    log::info!("Excel citit: {} facturi deschise, {} foi cu tabel", loaded.items.len(), loaded.sheets.iter().filter(|s| s.has_table).count());
                    file.loaded = Some(loaded);
                    file.mtime = mtime;
                    file.read_at = Some(Local::now());
                    file.error = None;
                }
                Ok(Err(message)) => {
                    log::warn!("Excel nu poate fi citit: {message}");
                    file.error = Some(message);
                }
                Err(join) => file.error = Some(join.to_string()),
            }
        }
        Some(_) => {}
    }
    rebuild_snapshot(app, &settings)
}

/// Recalculează ce se vede (de exemplu după schimbarea setărilor sau la trecerea într-o zi nouă), fără să recitească fișierul.
pub fn rebuild_snapshot(app: &AppHandle, settings: &Settings) -> Snapshot {
    let state = app.state::<AppState>();
    let snapshot = {
        let file = state.file.lock().unwrap();
        Snapshot::new(
            settings.excel_path.clone(),
            file.read_at.map(|t| t.format("%d.%m.%Y %H:%M").to_string()),
            file.error.clone(),
            file.loaded.as_ref(),
            settings,
            Local::now().date_naive(),
        )
    };
    // Interfața se redesenează doar dacă s-a schimbat ceva (altfel ar sări pagina la fiecare verificare).
    let changed = std::mem::replace(&mut *state.snapshot.lock().unwrap(), snapshot.clone()) != snapshot;
    if changed {
        let _ = app.emit("snapshot", &snapshot);
        tray::update_tooltip(app, &snapshot);
    }
    snapshot
}

fn needs_read(app: &AppHandle, path: &str, settings: &Settings) -> bool {
    let state = app.state::<AppState>();
    let file = state.file.lock().unwrap();
    if file.path.as_deref() != Some(path) || file.loaded.is_none() {
        return true;
    }
    let on_disk = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    if on_disk != file.mtime {
        return true;
    }
    let interval = chrono::Duration::minutes(i64::from(settings.check_minutes));
    file.read_at.map_or(true, |t| Local::now() - t >= interval)
}

fn read_file(path: &str) -> Result<(Loaded, Option<std::time::SystemTime>), String> {
    if !Path::new(path).exists() {
        return Err(format!("Nu găsesc fișierul „{path}”. A fost mutat sau redenumit?"));
    }
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let loaded = read_workbook(Path::new(path)).map_err(|e| e.to_string())?;
    Ok((loaded, mtime))
}

/// Dă alarma când a venit vremea ei: la interval (scadențe noi, mai urgente sau reamintiri) sau la ora fixă aleasă.
async fn maybe_alert(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let snapshot = state.snapshot.lock().unwrap().clone();
    let now = Local::now();

    let due = {
        let mut notify = state.notify.lock().unwrap();
        match settings.alert_mode {
            AlertMode::Interval => should_notify(&snapshot.alerts, &notify, now.timestamp(), now.hour(), &settings),
            AlertMode::Times => {
                // O oră fixă a sosit. Amânarea utilizatorului așteaptă (alerta vine după ea); altfel ora se
                // socotește tratată chiar dacă nu e nimic de spus, ca ceva apărut mai târziu să aștepte ora următoare.
                match due_slot(&settings.alert_minutes(), notify.last_slot, now.naive_local()) {
                    Some(slot) if now.timestamp() >= notify.snoozed_until => {
                        notify.last_slot = slot;
                        if snapshot.alerts.is_empty() {
                            if let Err(e) = persist::save_json(&state.notify_path(), &*notify) {
                                log::warn!("Nu pot salva starea alertelor: {e}");
                            }
                        }
                        !snapshot.alerts.is_empty()
                    }
                    _ => false,
                }
            }
        }
    };
    if !due {
        return;
    }
    {
        // Se notează înainte de trimitere, ca o trimitere lentă să nu ducă la o a doua alarmă.
        let mut notify = state.notify.lock().unwrap();
        mark_notified(&snapshot.alerts, &mut notify, now.timestamp());
        if let Err(e) = persist::save_json(&state.notify_path(), &*notify) {
            log::warn!("Nu pot salva starea alertelor: {e}");
        }
    }
    alert::fire(app, &snapshot.digest, &Channels::from_settings(&settings), None).await;
}
