use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Copy, Deserialize, Serialize)]
pub enum Madhab {
    #[default]
    Shafi,
    Hanafi,
}

impl From<Madhab> for salah::Madhab {
    fn from(value: Madhab) -> Self {
        match value {
            Madhab::Shafi => salah::Madhab::Shafi,
            Madhab::Hanafi => salah::Madhab::Hanafi,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Parameters {
    pub latitude: f64,
    pub longitude: f64,

    pub madhab: Madhab,
}

#[derive(Serialize)]
pub struct Timetable {
    pub(crate) fajr: chrono::NaiveTime,
    pub(crate) dhuhr: chrono::NaiveTime,
    pub(crate) asr: chrono::NaiveTime,
    pub(crate) maghrib: chrono::NaiveTime,
    pub(crate) isha: chrono::NaiveTime,
}

#[derive(Serialize)]
pub struct DayResponse {
    pub(crate) timetable: Timetable,
    pub(crate) current_date: NaiveDate,
    pub(crate) current_event: salah::Event,
}

#[derive(Serialize)]
pub struct MonthResponse {
    pub(crate) days: BTreeMap<NaiveDate, Timetable>,
    pub(crate) current_date: NaiveDate,
    pub(crate) total_days: u8,
}
