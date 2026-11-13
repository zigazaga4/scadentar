//! Citește un registru și scrie starea ca JSON (pentru previzualizarea interfeței).
//!
//!   cargo run -p scadentar-core --example dump_snapshot -- fisier.xls [AAAA-LL-ZZ]
//! Mesajul WhatsApp ce s-ar trimite apare pe stderr.
use std::path::Path;

use chrono::NaiveDate;
use scadentar_core::{read_workbook, Settings, Snapshot};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("dă calea fișierului Excel");
    let today = args
        .next()
        .map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").expect("data trebuie să fie AAAA-LL-ZZ"))
        .unwrap_or_else(|| chrono::Local::now().date_naive());
    let loaded = read_workbook(Path::new(&path)).expect("citire");
    let settings = Settings::default();
    let snapshot = Snapshot::new(Some(path), Some("13.11.2026 09:41".into()), None, Some(&loaded), &settings, today);
    eprintln!("{}\n", snapshot.digest.whatsapp);
    println!("{}", serde_json::to_string(&snapshot).unwrap());
}
