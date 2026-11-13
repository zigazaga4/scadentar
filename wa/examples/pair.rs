//! Încercare de la linia de comandă: pornește WhatsApp, afișează starea și (opțional) trimite un mesaj.
//!
//!   cargo run -p scadentar-wa --example pair -- <fișier-sesiune.db> [text de trimis]
use std::time::Duration;

use scadentar_wa::{WaService, WaStatus};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let db = args.next().expect("dă calea fișierului de sesiune");
    let text = args.next();
    let svc = WaService::new(db);
    let mut rx = svc.subscribe();
    if let Err(e) = svc.start().await {
        eprintln!("pornire eșuată: {e}");
        return;
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    loop {
        let status = rx.borrow_and_update().clone();
        match &status {
            WaStatus::Qr { image, expires_secs } => {
                println!("QR primit ({} octeți de imagine, valabil {expires_secs}s)", image.len());
                if text.is_none() {
                    break;
                }
            }
            WaStatus::Connected { phone } => {
                println!("CONECTAT: {phone:?}");
                if let Some(text) = &text {
                    println!("trimit: {:?}", svc.send_to_self(text).await);
                }
                break;
            }
            other => println!("stare: {other:?}"),
        }
        if tokio::time::timeout_at(deadline, rx.changed()).await.is_err() {
            println!("timp expirat");
            break;
        }
    }
    svc.stop().await;
}
