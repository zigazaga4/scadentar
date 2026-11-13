//! Comenzile apelate din interfață.

use chrono::Local;
use scadentar_core::format::{build_digest, Digest};
use scadentar_core::notify::{slot_stamp, snooze};
use scadentar_core::{Alertable, Scadenta, Settings, Snapshot, Urgency};
use scadentar_wa::WaStatus;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::alert::{self, Channels};
use crate::state::AppState;
use crate::{engine, persist, tray};

pub type CmdResult<T> = Result<T, String>;

/// Tot ce are nevoie fereastra principală când se deschide sau când se schimbă setările.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Boot {
    settings: Settings,
    snapshot: Snapshot,
    wa: WaStatus,
    wa_error: Option<String>,
    version: String,
    /// Versiune de dezvoltare: pornirea cu Windows nu se modifică.
    dev: bool,
}

fn boot(app: &AppHandle) -> Boot {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let snapshot = state.snapshot.lock().unwrap().clone();
    let wa_error = state.wa_error.lock().unwrap().clone();
    Boot { settings, snapshot, wa: state.wa.status(), wa_error, version: app.package_info().version.to_string(), dev: cfg!(debug_assertions) }
}

/// Modifică setările, le salvează și întoarce `true` dacă ceva s-a schimbat.
pub fn update_settings(app: &AppHandle, change: impl FnOnce(&mut Settings)) -> bool {
    let state = app.state::<AppState>();
    let mut settings = state.settings.lock().unwrap();
    let before = settings.clone();
    change(&mut settings);
    if *settings == before {
        return false;
    }
    if let Err(e) = persist::save_json(&state.settings_path(), &*settings) {
        log::warn!("Nu pot salva setările: {e}");
    }
    true
}

/// Pornirea cu Windows se aplică doar în versiunea instalată, nu în cea de dezvoltare.
pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    #[cfg(all(not(debug_assertions), not(any(target_os = "android", target_os = "ios"))))]
    {
        use tauri_plugin_autostart::ManagerExt;
        let launcher = app.autolaunch();
        if launcher.is_enabled().unwrap_or(false) != enabled {
            let result = if enabled { launcher.enable() } else { launcher.disable() };
            if let Err(e) = result {
                log::warn!("Nu pot schimba pornirea cu Windows: {e}");
            }
        }
    }
    #[cfg(any(debug_assertions, target_os = "android", target_os = "ios"))]
    let _ = (app, enabled);
}

#[tauri::command]
pub fn get_state(app: AppHandle) -> Boot {
    boot(&app)
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: Settings) -> CmdResult<Boot> {
    let settings = settings.sanitized();
    let mut schedule_changed = false;
    update_settings(&app, |current| {
        schedule_changed = current.alert_mode != settings.alert_mode || current.alert_times != settings.alert_times;
        let wa_paired = current.wa_paired; // starea legăturii cu WhatsApp o ține aplicația, nu interfața
        *current = Settings { wa_paired, ..settings.clone() };
    });
    if schedule_changed {
        // Orele de dinainte de schimbare nu mai contează: prima alertă vine la prima oră fixă care urmează.
        let state = app.state::<AppState>();
        let mut notify = state.notify.lock().unwrap();
        notify.last_slot = slot_stamp(Local::now().naive_local());
        if let Err(e) = persist::save_json(&state.notify_path(), &*notify) {
            log::warn!("Nu pot salva starea alertelor: {e}");
        }
    }
    apply_autostart(&app, settings.autostart);
    engine::refresh(&app, false).await;
    Ok(boot(&app))
}

/// Deschide dialogul de alegere a fișierului Excel; `None` dacă utilizatorul renunță.
#[tauri::command]
pub async fn choose_excel(app: AppHandle) -> CmdResult<Option<Boot>> {
    let dialog_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        dialog_app.dialog().file().set_title("Alege fișierul Excel cu facturile").add_filter("Excel", &["xls", "xlsx", "xlsm"]).blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(file) = picked else { return Ok(None) };
    let path = file.into_path().map_err(|e| e.to_string())?.to_string_lossy().into_owned();

    let first_time = update_settings(&app, |s| s.excel_path = Some(path.clone()));
    if first_time {
        let autostart = app.state::<AppState>().settings.lock().unwrap().autostart;
        apply_autostart(&app, autostart);
    }
    engine::refresh(&app, true).await;
    Ok(Some(boot(&app)))
}

/// Recitește fișierul acum, fără să aștepte următoarea verificare.
#[tauri::command]
pub async fn refresh_now(app: AppHandle) -> Snapshot {
    engine::refresh(&app, true).await
}

/// Deschide fișierul Excel în programul implicit.
#[tauri::command]
pub fn open_excel_file(app: AppHandle) -> CmdResult<()> {
    let path = app.state::<AppState>().settings.lock().unwrap().excel_path.clone().ok_or("Nu ai ales încă fișierul Excel.")?;
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    // `explorer <fișier>` deschide programul implicit și nu se încurcă în spații sau `&` din cale.
    #[cfg(target_os = "windows")]
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(&path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Declanșează o alertă de probă pe toate căile pornite. Normală: datele reale (sau un exemplu, dacă nu e
/// nimic scadent). Serioasă (`serious`): datele reale dacă au o scadență depășită la un contract important,
/// altfel un exemplu, ca să vezi cum arată.
#[tauri::command]
pub async fn test_alert(app: AppHandle, serious: bool) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let snapshot = state.snapshot.lock().unwrap().clone();
    let (digest, label) = if serious {
        let digest = if snapshot.digest.serious > 0 { snapshot.digest } else { sample_digest(true) };
        (digest, "🔔 *Test Scadențar* — așa arată o alertă SERIOASĂ")
    } else {
        let digest = if snapshot.digest.is_empty() { sample_digest(false) } else { snapshot.digest };
        (digest, "🔔 *Test Scadențar* — așa arată o alertă")
    };
    alert::fire(&app, &digest, &Channels::from_settings(&settings), Some(label)).await;
    Ok(())
}

/// Alertă de exemplu; cu `serious` conține și o rată depășită la un contract important.
fn sample_digest(serious: bool) -> Digest {
    let today = Local::now().date_naive();
    let make = |foaie: &str, factura: &str, days: i64, rest: f64, critical: bool| {
        let urgency = match days {
            d if d < 0 => Urgency::Overdue,
            0 => Urgency::Today,
            _ => Urgency::Soon,
        };
        Alertable {
            item: Scadenta { foaie: foaie.into(), factura: factura.into(), emisa: None, scadenta: today + chrono::Duration::days(days), suma: rest, rest, rand: 1 },
            urgency,
            days,
            critical,
        }
    };
    let mut items = vec![make("Exemplu Service", "FF 1042", 0, 830.5, false), make("Exemplu Asigurări", "RCA", 5, 1999.0, false)];
    items.push(if serious { make("Exemplu Leasing Mașină", "Rata 11", -2, 2450.0, true) } else { make("Exemplu Leasing", "Rata 11", -2, 2450.0, false) });
    build_digest(&items, today)
}

/// Ultima alertă, pentru fereastra pop-up.
#[tauri::command]
pub fn get_alert(app: AppHandle) -> Option<Digest> {
    app.state::<AppState>().digest.lock().unwrap().clone()
}

#[tauri::command]
pub fn dismiss_alert(app: AppHandle) {
    log::info!("Alerta a fost închisă din fereastră („Am văzut”).");
    alert::close_popup(&app);
}

/// Amână alertele cu `minutes` minute și închide fereastra de alertă.
#[tauri::command]
pub fn snooze_alerts(app: AppHandle, minutes: i64) {
    let state = app.state::<AppState>();
    {
        let mut notify = state.notify.lock().unwrap();
        snooze(&mut notify, Local::now().timestamp(), minutes.clamp(1, 24 * 60));
        if let Err(e) = persist::save_json(&state.notify_path(), &*notify) {
            log::warn!("Nu pot salva amânarea: {e}");
        }
    }
    alert::close_popup(&app);
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    tray::show_main(&app);
}
