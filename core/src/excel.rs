//! Citirea facturilor din registrul Excel (`.xls`, `.xlsx`, `.xlsm`).
//!
//! Registrul are o foaie pe furnizor. Fiecare foaie are un tabel cu antetul
//! „Nr crt | Factura fiscala nr. | Data emiterii | Data scadenta | Suma | Data plata | Suma plata | Rest de plata”.
//! Coloanele se găsesc după numele din antet (nu după poziție), fiindcă unele foi sunt mutate cu o coloană.
//! Datele sunt de obicei text (`10.04.2023`), nu date Excel; ambele variante sunt acceptate.

use std::io::Cursor;
use std::path::Path;

use calamine::{open_workbook_auto_from_rs, Data, Range, Reader};
use chrono::{Duration, NaiveDate};
use serde::Serialize;

use crate::model::Scadenta;

/// Cât se caută antetul, de sus în jos (numărul de rânduri).
const HEADER_SEARCH_ROWS: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum ExcelError {
    #[error("Nu pot citi fișierul „{0}”: {1}")]
    Read(String, String),
    #[error("Fișierul nu pare a fi un registru Excel valid: {0}")]
    Format(String),
}

/// Ce s-a găsit într-o foaie.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetInfo {
    pub name: String,
    /// Foaia are tabelul de facturi (altfel e ignorată: sumar, liste vechi etc.).
    pub has_table: bool,
    /// Facturi cu rest de plată și dată scadentă.
    pub open_items: usize,
    /// Facturi cu rest de plată dar fără dată scadentă (nu pot fi urmărite).
    pub without_due_date: usize,
}

/// Rezultatul citirii unui registru.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Loaded {
    pub items: Vec<Scadenta>,
    pub sheets: Vec<SheetInfo>,
    /// Rânduri cu rest de plată dar fără dată scadentă: nu pot fi urmărite.
    pub without_due_date: usize,
}

/// Citește registrul de la `path` și întoarce toate facturile cu rest de plată.
pub fn read_workbook(path: &Path) -> Result<Loaded, ExcelError> {
    // Se citește tot fișierul în memorie: merge și cât timp Excel îl are deschis.
    let bytes = std::fs::read(path).map_err(|e| ExcelError::Read(path.display().to_string(), e.to_string()))?;
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes)).map_err(|e| ExcelError::Format(e.to_string()))?;
    let mut loaded = Loaded::default();
    for name in workbook.sheet_names() {
        let Ok(range) = workbook.worksheet_range(&name) else {
            loaded.sheets.push(SheetInfo { name, has_table: false, open_items: 0, without_due_date: 0 });
            continue;
        };
        let parsed = parse_sheet(name.trim(), &table(&range));
        loaded.sheets.push(SheetInfo {
            name: name.trim().to_string(),
            has_table: parsed.has_table,
            open_items: parsed.items.len(),
            without_due_date: parsed.without_due_date,
        });
        loaded.without_due_date += parsed.without_due_date;
        loaded.items.extend(parsed.items);
    }
    loaded.items.sort_by(|a, b| a.scadenta.cmp(&b.scadenta).then_with(|| a.foaie.cmp(&b.foaie)).then(a.rand.cmp(&b.rand)));
    Ok(loaded)
}

/// Foaia ca tabel de celule cu pozițiile absolute (rândul 0 = primul rând din Excel).
pub type Table = Vec<Vec<Data>>;

fn table(range: &Range<Data>) -> Table {
    let Some((row0, col0)) = range.start() else { return Vec::new() };
    let mut rows: Table = vec![Vec::new(); row0 as usize];
    for row in range.rows() {
        let mut cells = vec![Data::Empty; col0 as usize];
        cells.extend(row.iter().cloned());
        rows.push(cells);
    }
    rows
}

#[derive(Debug, Default, PartialEq)]
pub struct ParsedSheet {
    pub has_table: bool,
    pub items: Vec<Scadenta>,
    pub without_due_date: usize,
}

/// Coloanele găsite în antet.
#[derive(Debug)]
struct Columns {
    header_row: usize,
    factura: usize,
    emisa: Option<usize>,
    scadenta: usize,
    suma: Option<usize>,
    data_plata: Option<usize>,
    suma_plata: Option<usize>,
    rest: usize,
}

/// Parsează o foaie. Public pentru teste cu foi construite pe loc.
pub fn parse_sheet(name: &str, rows: &Table) -> ParsedSheet {
    let Some(cols) = find_columns(rows) else { return ParsedSheet::default() };
    let mut out = ParsedSheet { has_table: true, ..Default::default() };
    for (index, row) in rows.iter().enumerate().skip(cols.header_row + 1) {
        let cell = |c: usize| row.get(c).unwrap_or(&Data::Empty);
        let suma = cols.suma.and_then(|c| number(cell(c)));
        let rest = rest_of(row, &cols, suma);
        if rest <= 0.0 {
            continue;
        }
        let Some(scadenta) = date(cell(cols.scadenta)) else {
            // Rândurile de total au rest dar nu au nici număr de factură, nici scadență: nu se numără.
            if !text(cell(cols.factura)).is_empty() {
                out.without_due_date += 1;
            }
            continue;
        };
        out.items.push(Scadenta {
            foaie: name.to_string(),
            factura: text(cell(cols.factura)),
            emisa: cols.emisa.and_then(|c| date(cell(c))),
            scadenta,
            suma: suma.unwrap_or(rest),
            rest,
            rand: index as u32 + 1,
        });
    }
    out
}

/// Cât mai e de plată pe un rând. Dacă celula „Rest de plată” e goală și nu s-a plătit nimic,
/// restul e toată suma facturii.
fn rest_of(row: &[Data], cols: &Columns, suma: Option<f64>) -> f64 {
    let cell = |c: usize| row.get(c).unwrap_or(&Data::Empty);
    if let Some(rest) = number(cell(cols.rest)) {
        return rest;
    }
    if !text(cell(cols.rest)).is_empty() {
        return 0.0; // text în loc de număr („ACHITAT”, „?”): nu ghicim
    }
    let nothing_paid = cols.data_plata.map_or(true, |c| text(cell(c)).is_empty())
        && cols.suma_plata.map_or(true, |c| number(cell(c)).unwrap_or(0.0) == 0.0);
    if nothing_paid {
        suma.unwrap_or(0.0)
    } else {
        0.0
    }
}

fn find_columns(rows: &Table) -> Option<Columns> {
    for (header_row, row) in rows.iter().enumerate().take(HEADER_SEARCH_ROWS) {
        let names: Vec<String> = row.iter().map(|c| normalize(&text(c))).collect();
        let find = |pred: &dyn Fn(&str) -> bool| names.iter().position(|n| pred(n));
        let (Some(factura), Some(scadenta), Some(rest)) =
            (find(&|n| n.contains("FACTUR")), find(&|n| n.contains("SCADEN")), find(&|n| n.starts_with("REST")))
        else {
            continue;
        };
        return Some(Columns {
            header_row,
            factura,
            emisa: find(&|n| n.contains("EMITER")),
            scadenta,
            suma: find(&|n| n == "SUMA"),
            data_plata: find(&|n| n.contains("DATA PLAT")),
            suma_plata: find(&|n| n.contains("SUMA PLAT")),
            rest,
        });
    }
    None
}

/// Majuscule fără diacritice, ca antetele să se potrivească oricum sunt scrise.
pub fn normalize(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            'ă' | 'Ă' | 'â' | 'Â' => 'A',
            'î' | 'Î' => 'I',
            'ș' | 'Ș' | 'ş' | 'Ş' => 'S',
            'ț' | 'Ț' | 'ţ' | 'Ţ' => 'T',
            c => c.to_ascii_uppercase(),
        })
        .collect()
}

fn text(cell: &Data) -> String {
    match cell {
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => s.trim().to_string(),
        Data::Float(f) => format_number(*f),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => excel_serial_date(dt.as_f64()).map(|d| d.format("%d.%m.%Y").to_string()).unwrap_or_default(),
        Data::Empty | Data::Error(_) => String::new(),
    }
}

fn format_number(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

/// Număr dintr-o celulă: numeric, sau text în format românesc (`1.234,56`) ori englezesc (`1234.56`).
pub fn number(cell: &Data) -> Option<f64> {
    match cell {
        Data::Float(f) => Some(*f),
        Data::Int(i) => Some(*i as f64),
        Data::String(s) => parse_number(s),
        _ => None,
    }
}

pub fn parse_number(s: &str) -> Option<f64> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace() && *c != '\u{a0}').collect();
    if cleaned.is_empty() || !cleaned.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if !cleaned.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+')) {
        return None;
    }
    let normalized = match (cleaned.rfind(','), cleaned.rfind('.')) {
        (Some(comma), Some(dot)) if comma > dot => cleaned.replace('.', "").replace(',', "."),
        (Some(_), Some(_)) => cleaned.replace(',', ""),
        (Some(_), None) => {
            if cleaned.matches(',').count() > 1 {
                cleaned.replace(',', "")
            } else {
                cleaned.replace(',', ".")
            }
        }
        (None, Some(_)) if cleaned.matches('.').count() > 1 => cleaned.replace('.', ""),
        _ => cleaned,
    };
    normalized.parse().ok()
}

/// Dată dintr-o celulă: dată Excel, număr de serie sau text (`10.04.2023`, `10/4/23`, `2023-04-10`).
pub fn date(cell: &Data) -> Option<NaiveDate> {
    match cell {
        Data::DateTime(dt) => excel_serial_date(dt.as_f64()),
        Data::Float(f) if (20_000.0..80_000.0).contains(f) => excel_serial_date(*f),
        Data::Int(i) if (20_000..80_000).contains(i) => excel_serial_date(*i as f64),
        Data::String(s) | Data::DateTimeIso(s) => parse_date(s),
        _ => None,
    }
}

fn excel_serial_date(serial: f64) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(1899, 12, 30)?.checked_add_signed(Duration::days(serial.floor() as i64))
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    let date_part = s.split(['T', ' ']).next()?;
    let parts: Vec<&str> = date_part.split(['.', '/', '-']).collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let n: Vec<u32> = parts.iter().map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let (year, month, day) = if parts[0].len() == 4 { (n[0], n[1], n[2]) } else { (n[2], n[1], n[0]) };
    let year = if year < 100 { 2000 + year } else { year };
    NaiveDate::from_ymd_opt(year as i32, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Data {
        Data::String(v.to_string())
    }

    fn sheet(rows: Vec<Vec<Data>>) -> Table {
        rows
    }

    fn header() -> Vec<Data> {
        ["", "Nr crt", "FACTURA FISCALA NR.", "DATA EMITERII", "DATA SCADENTA", "SUMA", "DATA PLATA", "SUMA PLATA", "REST DE PLATA", "TRIMIS CONTA"]
            .iter()
            .map(|h| if h.is_empty() { Data::Empty } else { s(h) })
            .collect()
    }

    fn invoice(nr: &str, due: &str, suma: f64, paid_on: &str, paid: Option<f64>, rest: Option<f64>) -> Vec<Data> {
        vec![
            Data::Empty,
            Data::Float(1.0),
            s(nr),
            s("01.01.2026"),
            s(due),
            Data::Float(suma),
            if paid_on.is_empty() { Data::Empty } else { s(paid_on) },
            paid.map_or(Data::Empty, Data::Float),
            rest.map_or(Data::Empty, Data::Float),
            Data::Empty,
        ]
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn dates_in_all_the_shapes_the_register_uses() {
        assert_eq!(parse_date("10.04.2023"), Some(d(2023, 4, 10)));
        assert_eq!(parse_date(" 5.4.2023 "), Some(d(2023, 4, 5)));
        assert_eq!(parse_date("05/04/23"), Some(d(2023, 4, 5)));
        assert_eq!(parse_date("2023-04-10"), Some(d(2023, 4, 10)));
        assert_eq!(parse_date("2023-04-10T00:00:00"), Some(d(2023, 4, 10)));
        assert_eq!(parse_date("SCADENT 12.07.2024"), None);
        assert_eq!(parse_date("31.02.2024"), None);
        assert_eq!(parse_date(""), None);
        assert_eq!(date(&Data::Float(45000.0)), Some(d(2023, 3, 15)));
        assert_eq!(date(&Data::Float(12.0)), None);
    }

    #[test]
    fn numbers_in_romanian_and_english_formats() {
        assert_eq!(parse_number("1.234,56"), Some(1234.56));
        assert_eq!(parse_number("1,234.56"), Some(1234.56));
        assert_eq!(parse_number("1234.56"), Some(1234.56));
        assert_eq!(parse_number("1234,56"), Some(1234.56));
        assert_eq!(parse_number("1 234,5"), Some(1234.5));
        assert_eq!(parse_number("1.234.567"), Some(1234567.0));
        assert_eq!(parse_number("-18.61"), Some(-18.61));
        assert_eq!(parse_number("?"), None);
        assert_eq!(parse_number("ACHITAT 32867.25"), None);
        assert_eq!(parse_number(""), None);
    }

    #[test]
    fn header_is_found_by_name_even_when_shifted() {
        let mut shifted = header();
        shifted.insert(1, Data::Empty);
        let mut row = invoice("F1", "20.11.2026", 100.0, "", None, Some(100.0));
        row.insert(1, Data::Empty);
        let parsed = parse_sheet("Volvo", &sheet(vec![vec![], vec![s("Furnizor Volvo")], shifted, row]));
        assert!(parsed.has_table);
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.items[0].scadenta, d(2026, 11, 20));
        assert_eq!(parsed.items[0].rand, 4);
    }

    #[test]
    fn sheets_without_the_invoice_table_are_ignored() {
        let summary = sheet(vec![vec![s("Nr crt"), s("Furnizor"), s("Suma"), s("Data scadenta"), s("Achitat")], vec![Data::Float(1.0), s("IMPULS"), Data::Float(10.0)]]);
        let parsed = parse_sheet("Lista plata", &summary);
        assert!(!parsed.has_table);
        assert!(parsed.items.is_empty());
    }

    #[test]
    fn paid_invoices_total_rows_and_pennies_are_not_open_items() {
        let rows = sheet(vec![
            header(),
            invoice("PAID", "10.04.2026", 500.0, "05.04.2026", Some(500.0), Some(0.0)),
            invoice("OPEN", "10.04.2026", 500.0, "", None, Some(500.0)),
            invoice("PART", "11.04.2026", 500.0, "05.04.2026", Some(200.0), Some(300.0)),
            // rest lipsă și nimic plătit: toată suma e de plată
            invoice("BLANK", "12.04.2026", 80.0, "", None, None),
            // rest lipsă dar plătit: nu e de plată
            invoice("BLANKPAID", "12.04.2026", 80.0, "01.04.2026", Some(80.0), None),
            // credit (rest negativ): nu e de plată
            invoice("CREDIT", "12.04.2026", -18.61, "", None, Some(-18.61)),
            // rând de total: fără număr de factură și fără scadență
            vec![Data::Empty, Data::Empty, Data::Empty, Data::Empty, Data::Empty, Data::Float(9999.0), Data::Empty, Data::Empty, Data::Float(9999.0)],
        ]);
        let parsed = parse_sheet("Furnizor", &rows);
        let names: Vec<&str> = parsed.items.iter().map(|i| i.factura.as_str()).collect();
        assert_eq!(names, ["OPEN", "PART", "BLANK"]);
        assert_eq!(parsed.items[1].rest, 300.0);
        assert_eq!(parsed.items[2].rest, 80.0);
        assert_eq!(parsed.without_due_date, 0);
    }

    #[test]
    fn open_invoice_without_due_date_is_counted_but_not_listed() {
        let rows = sheet(vec![header(), invoice("NODATE", "", 100.0, "", None, Some(100.0))]);
        let parsed = parse_sheet("Furnizor", &rows);
        assert!(parsed.items.is_empty());
        assert_eq!(parsed.without_due_date, 1);
    }

    #[test]
    fn real_register_is_read_when_available() {
        // Rulează doar cu `SCADENTAR_XLS=/cale/fisier.xls cargo test -p scadentar-core -- --nocapture`.
        let Some(path) = std::env::var_os("SCADENTAR_XLS") else { return };
        let loaded = read_workbook(Path::new(&path)).expect("citire");
        let with_table = loaded.sheets.iter().filter(|s| s.has_table).count();
        println!("{} foi cu tabel din {}, {} facturi deschise, {} fără scadență", with_table, loaded.sheets.len(), loaded.items.len(), loaded.without_due_date);
        assert!(with_table > 50);
        assert!(!loaded.items.is_empty());
    }
}
