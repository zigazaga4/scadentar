//! Cum ajunge alerta la om: notificare Windows, fereastră în prim-plan cu sunet, mesaj WhatsApp.

use scadentar_core::format::Digest;
use scadentar_core::Settings;
use tauri::{AppHandle, Emitter, Manager, UserAttentionType, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;

use crate::sound;
use crate::state::AppState;

pub const ALERT_WINDOW: &str = "alert";

/// Pe ce căi se transmite alerta.
#[derive(Clone, Copy, Debug)]
pub struct Channels {
    pub toast: bool,
    pub popup: bool,
    pub sound: bool,
    pub whatsapp: bool,
}

impl Channels {
    pub fn from_settings(s: &Settings) -> Self {
        Self { toast: s.notify_toast, popup: s.notify_popup, sound: s.notify_sound, whatsapp: s.notify_whatsapp }
    }
}

/// Transmite alerta pe toate căile pornite. `whatsapp_prefix` se pune în fața mesajului WhatsApp (ex. „Test”).
pub async fn fire(app: &AppHandle, digest: &Digest, channels: &Channels, whatsapp_prefix: Option<&str>) {
    let state = app.state::<AppState>();
    *state.digest.lock().unwrap() = Some(digest.clone());
    log::info!("Alertă: {} ({} linii), căi: {channels:?}", digest.title, digest.groups.len());

    if channels.toast {
        let shown = app.notification().builder().title(format!("Scadențe de plată: {}", digest.title)).body(&digest.toast).show();
        if let Err(e) = shown {
            log::warn!("Notificarea Windows nu a putut fi afișată: {e}");
        }
    }
    if channels.popup {
        show_popup(app, digest);
    }
    if channels.sound {
        sound::alarm();
    }
    if channels.whatsapp {
        send_whatsapp(app, digest, whatsapp_prefix).await;
    }
}

/// Deschide (sau aduce în față) fereastra de alertă, mereu deasupra celorlalte ferestre.
pub fn show_popup(app: &AppHandle, digest: &Digest) {
    if let Some(window) = app.get_webview_window(ALERT_WINDOW) {
        let _ = app.emit_to(ALERT_WINDOW, "alert-updated", digest);
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        let _ = window.request_user_attention(Some(UserAttentionType::Critical));
        return;
    }
    let built = WebviewWindowBuilder::new(app, ALERT_WINDOW, WebviewUrl::App("alert.html".into()))
        .title("Scadențe de plată")
        .inner_size(680.0, 780.0)
        .min_inner_size(480.0, 420.0)
        .center()
        .always_on_top(true)
        .focused(true)
        .build();
    match built {
        Ok(window) => {
            log::info!("Fereastra de alertă: vizibilă={:?}, dimensiune={:?}, poziție={:?}", window.is_visible(), window.outer_size().ok(), window.outer_position().ok());
            let _ = window.request_user_attention(Some(UserAttentionType::Critical));
        }
        Err(e) => log::warn!("Fereastra de alertă nu s-a putut deschide: {e}"),
    }
}

pub fn close_popup(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(ALERT_WINDOW) {
        let _ = window.close();
    }
}

/// Trimite rezumatul pe WhatsApp, în chatul „Mesaj către tine”. Dacă nu s-a făcut niciodată legătura
/// cu WhatsApp, nu face nimic; dacă legătura există dar nu merge, păstrează eroarea ca s-o vadă omul.
async fn send_whatsapp(app: &AppHandle, digest: &Digest, prefix: Option<&str>) {
    let state = app.state::<AppState>();
    if !state.settings.lock().unwrap().wa_paired {
        return;
    }
    let text = match prefix {
        Some(prefix) => format!("{prefix}\n\n{}", digest.whatsapp),
        None => digest.whatsapp.clone(),
    };
    let result = state.wa.send_to_self(&text).await;
    let message = result.err();
    if let Some(m) = &message {
        log::warn!("Mesajul WhatsApp nu a plecat: {m}");
    }
    *state.wa_error.lock().unwrap() = message.clone();
    let _ = app.emit("wa-error", message);
}
