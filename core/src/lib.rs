//! Logica aplicației Scadențar, fără interfață: citirea facturilor din Excel, regulile de
//! scadență, deciderea când se trimite o alertă și textele mesajelor.

pub mod excel;
pub mod format;
pub mod model;
pub mod notify;
pub mod settings;
pub mod snapshot;

pub use excel::{read_workbook, Loaded, SheetInfo};
pub use model::{Alertable, Group, Scadenta, Urgency};
pub use notify::{classify, group, mark_notified, should_notify, Classified, NotifyState};
pub use settings::Settings;
pub use snapshot::Snapshot;
