//! Iconița din zona de notificări (tray) și arătarea ferestrei principale.

use scadentar_core::Snapshot;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::engine;

const TRAY_ID: &str = "main";

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Deschide Scadențar", true, None::<&str>)?;
    let check = MenuItem::with_id(app, "check", "Verifică acum", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Ieșire", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &check, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Scadențar")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "check" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move { engine::refresh(&app, true).await });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Textul de deasupra iconiței: câte scadențe sunt în alertă.
pub fn update_tooltip(app: &AppHandle, snapshot: &Snapshot) {
    let d = &snapshot.digest;
    let text = if snapshot.file.is_none() {
        "Scadențar — alege fișierul Excel".to_string()
    } else if d.is_empty() {
        "Scadențar — nicio scadență apropiată".to_string()
    } else {
        format!("Scadențar — {}", d.title)
    };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}

/// Arată fereastra principală și o aduce în față.
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
