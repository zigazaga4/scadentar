//! Conexiunea WhatsApp a aplicației: autentificare prin cod QR, sesiune salvată pe disc
//! și trimiterea de mesaje către propriul număr (chatul „Mesaj către tine”).
//!
//! Biblioteca de sub capotă este `whatsapp-rust` (client neoficial WhatsApp Web, multi-device).
//! Starea se citește cu [`WaService::status`] sau se urmărește cu [`WaService::subscribe`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use serde::Serialize;
use tokio::sync::{watch, Mutex};
use whatsapp_rust::prelude::*;
use whatsapp_rust::waproto::whatsapp as wa;

/// Ce vede utilizatorul despre conexiune.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum WaStatus {
    /// Nu rulează (nu a fost pornit sau a fost oprit).
    Stopped,
    /// Se stabilește conexiunea.
    Starting,
    /// Așteaptă scanarea codului QR; `image` este un `data:` URL cu imaginea codului.
    #[serde(rename_all = "camelCase")]
    Qr { image: String, expires_secs: u64 },
    /// Conectat și autentificat; `phone` e numărul contului, dacă îl știm.
    Connected { phone: Option<String> },
    /// Dispozitivul a fost deconectat de pe telefon (sau prin „Deconectează”).
    LoggedOut,
    /// Eroare la pornire sau pe parcurs.
    Error { message: String },
}

impl WaStatus {
    pub fn is_connected(&self) -> bool {
        matches!(self, WaStatus::Connected { .. })
    }

    /// Numele stării, fără conținut (pentru jurnal: codul QR e foarte lung).
    pub fn name(&self) -> &'static str {
        match self {
            WaStatus::Stopped => "oprit",
            WaStatus::Starting => "se conectează",
            WaStatus::Qr { .. } => "așteaptă codul QR",
            WaStatus::Connected { .. } => "conectat",
            WaStatus::LoggedOut => "deconectat",
            WaStatus::Error { .. } => "eroare",
        }
    }
}

/// Imaginea SVG a unui cod QR, ca `data:` URL (se pune direct într-un `<img src>`).
pub fn qr_data_url(payload: &str) -> Result<String, String> {
    use qrcode::render::svg;
    let code = qrcode::QrCode::new(payload.as_bytes()).map_err(|e| e.to_string())?;
    let svg = code
        .render::<svg::Color>()
        .min_dimensions(280, 280)
        .quiet_zone(true)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    let b64 = base64::engine::general_purpose::STANDARD.encode(svg);
    Ok(format!("data:image/svg+xml;base64,{b64}"))
}

struct Running {
    handle: BotHandle,
}

struct Inner {
    db_path: PathBuf,
    status: watch::Sender<WaStatus>,
    running: Mutex<Option<Running>>,
}

/// Serviciul WhatsApp. Se clonează ieftin (Arc) și se poate folosi din mai multe task-uri.
#[derive(Clone)]
pub struct WaService {
    inner: Arc<Inner>,
}

impl WaService {
    /// `db_path` este fișierul SQLite în care se păstrează sesiunea (cheile dispozitivului).
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        let (status, _) = watch::channel(WaStatus::Stopped);
        Self { inner: Arc::new(Inner { db_path: db_path.into(), status, running: Mutex::new(None) }) }
    }

    pub fn status(&self) -> WaStatus {
        self.inner.status.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<WaStatus> {
        self.inner.status.subscribe()
    }

    fn set(&self, status: WaStatus) {
        self.inner.status.send_replace(status);
    }

    /// Pornește conexiunea. Dacă există o sesiune salvată se conectează direct; altfel apare un
    /// cod QR (starea `Qr`). Dacă rulează deja, nu face nimic.
    pub async fn start(&self) -> Result<(), String> {
        let mut running = self.inner.running.lock().await;
        if running.is_some() {
            return Ok(());
        }
        self.set(WaStatus::Starting);
        if let Some(dir) = self.inner.db_path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("Nu pot crea folderul pentru WhatsApp: {e}"))?;
        }
        let db = self.inner.db_path.to_string_lossy().into_owned();
        let store = SqliteStore::new(&db).await.map_err(|e| self.fail(format!("Nu pot deschide sesiunea WhatsApp: {e}")))?;

        let on_qr = self.clone();
        let on_ok = self.clone();
        let on_out = self.clone();
        let bot = Bot::builder()
            .with_backend(store)
            .skip_history_sync()
            .with_device_props(
                whatsapp_rust::wacore::store::DevicePropsOverride::new()
                    .with_os("Scadențar")
                    .with_platform_type(wa::device_props::PlatformType::Desktop),
            )
            .on_qr_code(move |code, timeout| {
                let svc = on_qr.clone();
                async move {
                    match qr_data_url(&code) {
                        Ok(image) => svc.set(WaStatus::Qr { image, expires_secs: timeout.as_secs() }),
                        Err(e) => svc.set(WaStatus::Error { message: format!("Cod QR invalid: {e}") }),
                    }
                }
            })
            .on_connected(move |client| {
                let svc = on_ok.clone();
                async move {
                    let phone = client.pn().map(|jid| format!("+{}", jid.user));
                    svc.set(WaStatus::Connected { phone });
                }
            })
            .on_logged_out(move |_info| {
                let svc = on_out.clone();
                async move {
                    svc.set(WaStatus::LoggedOut);
                    // Din alt task: oprirea botului nu are voie să se facă chiar din handler-ul lui.
                    tokio::spawn(async move { svc.release_after_logout().await });
                }
            })
            .build()
            .await
            .map_err(|e| self.fail(format!("Nu pot porni WhatsApp: {e}")))?;

        *running = Some(Running { handle: bot.spawn() });
        Ok(())
    }

    /// Dispozitivul a fost deconectat (de pe telefon): elibereză botul și șterge sesiunea, ca
    /// următorul `start` să ceară din nou codul QR.
    async fn release_after_logout(&self) {
        let running = self.inner.running.lock().await.take();
        drop(running);
        remove_session_files(&self.inner.db_path);
    }

    fn fail(&self, message: String) -> String {
        self.set(WaStatus::Error { message: message.clone() });
        message
    }

    /// Oprește conexiunea, dar păstrează sesiunea: la următorul `start` nu mai cere codul QR.
    pub async fn stop(&self) {
        let running = self.inner.running.lock().await.take();
        if let Some(running) = running {
            running.handle.shutdown().await;
        }
        if !matches!(self.status(), WaStatus::LoggedOut) {
            self.set(WaStatus::Stopped);
        }
    }

    /// Deconectează dispozitivul din contul WhatsApp și șterge sesiunea de pe acest calculator.
    pub async fn logout(&self) {
        let running = self.inner.running.lock().await.take();
        if let Some(running) = running {
            running.handle.client().logout().await;
            running.handle.shutdown().await;
        }
        remove_session_files(&self.inner.db_path);
        self.set(WaStatus::LoggedOut);
    }

    /// Trimite `text` în chatul „Mesaj către tine” (propriul număr). Cere conexiune activă.
    pub async fn send_to_self(&self, text: &str) -> Result<(), String> {
        let client = {
            let running = self.inner.running.lock().await;
            running.as_ref().map(|r| r.handle.client())
        };
        let client = client.ok_or("WhatsApp nu este pornit.")?;
        if !self.status().is_connected() || !client.is_connected() {
            return Err("WhatsApp nu este conectat.".into());
        }
        let own = client.pn().ok_or("Nu știu numărul contului WhatsApp (încă nu e autentificat).")?;
        client
            .send_message(own.to_non_ad(), wa::Message::text(text))
            .await
            .map(|_| ())
            .map_err(|e| format!("Mesajul nu a putut fi trimis: {e}"))
    }
}

/// Șterge fișierele sesiunii SQLite (baza, jurnalul WAL și fișierul de memorie partajată).
fn remove_session_files(db_path: &Path) {
    let base = db_path.to_string_lossy().into_owned();
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(format!("{base}{suffix}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_is_a_valid_svg_data_url() {
        let url = qr_data_url("2@abcdef,ghijkl,mnopqr,stuvwx==").unwrap();
        assert!(url.starts_with("data:image/svg+xml;base64,"));
        let b64 = url.trim_start_matches("data:image/svg+xml;base64,");
        let svg = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(b64).unwrap()).unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn status_serializes_with_a_state_tag() {
        let json = serde_json::to_string(&WaStatus::Connected { phone: Some("+40700000000".into()) }).unwrap();
        assert_eq!(json, r#"{"state":"connected","phone":"+40700000000"}"#);
        let json = serde_json::to_string(&WaStatus::Qr { image: "x".into(), expires_secs: 20 }).unwrap();
        assert_eq!(json, r#"{"state":"qr","image":"x","expiresSecs":20}"#);
    }
}
