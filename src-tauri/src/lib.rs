//! Scadențar: aplicație care stă în fundal, citește registrul de facturi din Excel și avertizează
//! (notificare Windows, fereastră cu sunet, WhatsApp) când se apropie sau a trecut o scadență.

mod alert;
mod commands;
mod engine;
mod persist;
mod sound;
mod state;
mod tray;
mod update;
mod whatsapp;

use std::sync::Mutex;

use scadentar_core::notify::NotifyState;
use scadentar_core::{Settings, Snapshot};
use scadentar_wa::WaService;
use tauri::{Manager, RunEvent, WindowEvent};

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Prima: o a doua pornire doar aduce în față fereastra aplicației deja pornite.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_main(app)))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Warn)
                .level_for("scadentar_lib", log::LevelFilter::Info)
                .level_for("scadentar_wa", log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: Some("scadentar".into()) }),
                ])
                .max_file_size(1_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--background"])))
        .setup(|app| {
            let handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            std::fs::create_dir_all(&data_dir)?;

            let settings: Settings = persist::load_json::<Settings>(&config_dir.join("setari.json")).sanitized();
            let notify: NotifyState = persist::load_json(&config_dir.join("stare.json"));
            let background = std::env::args().any(|a| a == "--background");
            let configured = settings.excel_path.is_some();
            let autostart = settings.autostart;
            let snapshot = Snapshot::new(settings.excel_path.clone(), None, None, None, &settings, chrono::Local::now().date_naive());

            app.manage(AppState {
                config_dir,
                settings: Mutex::new(settings),
                notify: Mutex::new(notify),
                file: Mutex::new(Default::default()),
                snapshot: Mutex::new(snapshot),
                digest: Mutex::new(None),
                wa_error: Mutex::new(None),
                wa: WaService::new(data_dir.join("whatsapp").join("sesiune.db")),
                refreshing: tokio::sync::Mutex::new(()),
            });

            tray::build(&handle)?;
            // Pornită de Windows în fundal (și deja configurată): rămâne doar în tray. Altfel se deschide fereastra.
            if !(background && configured) {
                tray::show_main(&handle);
            }
            if configured {
                commands::apply_autostart(&handle, autostart);
            }
            whatsapp::spawn_status_forwarder(handle.clone());
            whatsapp::auto_start(handle.clone());
            engine::spawn_scheduler(handle.clone());
            update::spawn_background_check(handle);
            Ok(())
        })
        // „X” ascunde fereastra principală; aplicația rămâne în fundal și se închide doar din tray.
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed) {
                log::info!("Fereastra „{}”: {event:?}", window.label());
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::save_settings,
            commands::choose_excel,
            commands::refresh_now,
            commands::open_excel_file,
            commands::test_alert,
            commands::get_alert,
            commands::dismiss_alert,
            commands::snooze_alerts,
            commands::show_main_window,
            whatsapp::wa_connect,
            whatsapp::wa_disconnect,
            whatsapp::wa_send_test,
            update::app_version,
            update::check_update,
            update::install_update
        ])
        .build(tauri::generate_context!())
        .expect("eroare la pornirea aplicației");

    app.run(|_app, event| {
        // Închiderea ultimei ferestre (de ex. a alertei) nu oprește aplicația; „Ieșire” din tray da.
        if let RunEvent::ExitRequested { api, code, .. } = event {
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}
