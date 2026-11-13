//! Legătura dintre serviciul WhatsApp și aplicație: urmărirea stării, pornirea automată, comenzile din interfață.

use scadentar_wa::WaStatus;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::{update_settings, CmdResult};
use crate::state::AppState;

/// Trimite interfața starea WhatsApp de fiecare dată când se schimbă și ține minte dacă
/// legătura cu contul există (ca la pornire aplicația să se reconecteze singură).
pub fn spawn_status_forwarder(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut rx = app.state::<AppState>().wa.subscribe();
        while rx.changed().await.is_ok() {
            let status = rx.borrow_and_update().clone();
            log::info!("WhatsApp: {}", status.name());
            let _ = app.emit("wa-status", &status);
            match status {
                WaStatus::Connected { .. } => {
                    update_settings(&app, |s| s.wa_paired = true);
                    *app.state::<AppState>().wa_error.lock().unwrap() = None;
                }
                WaStatus::LoggedOut => {
                    update_settings(&app, |s| s.wa_paired = false);
                }
                _ => {}
            }
        }
    });
}

/// Dacă s-a făcut deja legătura cu WhatsApp, o reia la pornirea aplicației.
pub fn auto_start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if state.settings.lock().unwrap().wa_paired {
            if let Err(e) = state.wa.start().await {
                log::warn!("WhatsApp nu s-a putut reconecta: {e}");
            }
        }
    });
}

/// Pornește conexiunea; dacă nu există sesiune apare codul QR.
#[tauri::command]
pub async fn wa_connect(app: AppHandle) -> CmdResult<()> {
    app.state::<AppState>().wa.start().await
}

/// Deconectează dispozitivul din cont și șterge sesiunea de pe acest calculator.
#[tauri::command]
pub async fn wa_disconnect(app: AppHandle) -> CmdResult<()> {
    app.state::<AppState>().wa.logout().await;
    update_settings(&app, |s| s.wa_paired = false);
    Ok(())
}

/// Un mesaj de probă în chatul „Mesaj către tine”.
#[tauri::command]
pub async fn wa_send_test(app: AppHandle) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let result = state.wa.send_to_self("✅ *Scadențar* este conectat la WhatsApp. Aici vei primi alertele despre facturile și ratele scadente.").await;
    *state.wa_error.lock().unwrap() = result.clone().err();
    let _ = app.emit("wa-error", result.clone().err());
    result
}
