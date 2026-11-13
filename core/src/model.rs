use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// O factură neachitată (sau achitată parțial), cu data scadentă.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scadenta {
    /// Furnizorul (numele foii din Excel).
    pub foaie: String,
    /// Numărul facturii / al ratei, așa cum e scris în Excel (poate lipsi).
    pub factura: String,
    pub emisa: Option<NaiveDate>,
    pub scadenta: NaiveDate,
    /// Valoarea totală a facturii.
    pub suma: f64,
    /// Cât mai e de plată.
    pub rest: f64,
    /// Rândul din foaie (începe de la 1), ca să poată fi găsită ușor în Excel.
    pub rand: u32,
}

/// Cât de urgentă e o scadență.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Urgency {
    /// Scade în următoarele zile.
    Soon = 0,
    /// Scade azi.
    Today = 1,
    /// Data scadentă a trecut.
    Overdue = 2,
}

impl Urgency {
    pub fn level(self) -> u8 {
        self as u8
    }
}

/// O scadență împreună cu poziția ei față de ziua de azi.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alertable {
    #[serde(flatten)]
    pub item: Scadenta,
    pub urgency: Urgency,
    /// Zile până la scadență: negativ = depășită cu atâtea zile.
    pub days: i64,
}

/// Facturile aceluiași furnizor cu aceeași dată scadentă (ex. 10 rate de leasing în aceeași zi).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub foaie: String,
    pub scadenta: NaiveDate,
    pub urgency: Urgency,
    pub days: i64,
    pub count: usize,
    pub total_rest: f64,
    pub facturi: Vec<String>,
}
