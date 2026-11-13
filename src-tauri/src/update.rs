//! Actualizări automate: aplicația caută pe GitHub o versiune mai nouă (vezi `plugins.updater` în tauri.conf.json).

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::alert::ALERT_WINDOW;
use crate::commands::CmdResult;

/// Cât de des se caută o versiune nouă cât timp aplicația rulează în fundal.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(45);

#[derive(Serialize)]
pub struct UpdateInfo {
    version: String,
    notes: Option<String>,
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Versiunea mai nouă, dacă există. Lipsa internetului sau a unei versiuni publicate vine ca text de eroare.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> CmdResult<Option<UpdateInfo>> {
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    Ok(update.map(|u| UpdateInfo { version: u.version, notes: u.body }))
}

/// Descarcă versiunea nouă, o instalează și repornește aplicația.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> CmdResult<()> {
    install_if_available(&app).await.map(|_| ())
}

async fn install_if_available(app: &AppHandle) -> CmdResult<bool> {
    let Some(update) = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())? else {
        return Ok(false);
    };
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    app.restart()
}

/// Caută periodic o versiune nouă și o instalează singură, dar nu în timp ce o alertă e pe ecran.
pub fn spawn_background_check(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_AFTER).await;
        loop {
            if app.get_webview_window(ALERT_WINDOW).is_none() {
                if let Err(e) = install_if_available(&app).await {
                    log::info!("Verificarea actualizărilor: {e}");
                }
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}
