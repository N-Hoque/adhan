use std::collections::BTreeMap;

use chrono::NaiveDate;
use rocket::{
    form::{Form, Strict},
    FromForm, FromFormField,
};
use serde::Serialize;

#[derive(Default, Debug, Clone, Copy, FromFormField)]
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

#[derive(Debug, FromForm)]
pub struct FormParameters {
    pub(crate) latitude: Strict<f64>,
    pub(crate) longitude: Strict<f64>,

    pub(crate) madhab: Madhab,
}

#[derive(Clone)]
pub(crate) struct Parameters {
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,

    pub(crate) madhab: Madhab,
}

impl From<Form<FormParameters>> for Parameters {
    fn from(value: Form<FormParameters>) -> Self {
        Self {
            latitude: *value.latitude,
            longitude: *value.longitude,
            madhab: value.madhab,
        }
    }
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
