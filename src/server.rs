use rocket::{form::Form, get, post, FromForm, FromFormField};
use salah::{Coordinates, Local};

#[derive(Default, Clone, Copy, FromFormField)]
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

#[derive(FromForm)]
pub struct Parameters {
    latitude: f64,
    longitude: f64,

    madhab: Madhab,
}

pub struct Timetable<Tz: salah::TimeZone> {
    fajr: chrono::DateTime<Tz>,
    dhuhr: chrono::DateTime<Tz>,
    asr: chrono::DateTime<Tz>,
    maghrib: chrono::DateTime<Tz>,
    isha: chrono::DateTime<Tz>,
}

pub struct DayResponse<Tz: salah::TimeZone> {
    timetable: Timetable<Tz>,
    current_prayer: salah::Prayer,
    is_restricted: bool,
}

pub struct MonthResponse<Tz: salah::TimeZone> {
    days: Vec<DayResponse<Tz>>,
    current_day: u8,
    total_days: u8,
    current_month: u8,
}

#[get("/")]
fn index() -> &'static str {
    "Welcome to Adhan!"
}

#[post("/times/now", data = "<parameters>")]
fn new_current_timetable(parameters: Option<Form<Parameters>>) -> Result<DayResponse<Local>, String> {
    let mut schedule = salah::Schedule::<Local>::now();
    let mut s = &mut schedule;
    if let Some(parameters) = parameters {
        s = s
            .with_coordinates(Coordinates::new(parameters.latitude, parameters.longitude))
            .with_parameters(
                salah::Parameters::from_method(salah::Method::MuslimWorldLeague).with_madhab(parameters.madhab.into()),
            );
    }
    s.build()
}

#[post("/times/<day>", data = "<parameters>")]
fn new_daily_timetable(day: u8, parameters: Option<Form<Parameters>>) -> Result<DayResponse<Local>, String> {
    let mut schedule = salah::Schedule::<Local>::now();
    let mut s = &mut schedule;
    if let Some(parameters) = parameters {
        s = s
            .with_coordinates(Coordinates::new(parameters.latitude, parameters.longitude))
            .with_parameters(
                salah::Parameters::from_method(salah::Method::MuslimWorldLeague).with_madhab(parameters.madhab.into()),
            );
    }
    s.build()
}

#[post("/times/month/<month>", data = "<parameters>")]
fn new_monthly_timetable(month: u8, parameters: Option<Form<Parameters>>) -> Result<DayResponse<Local>, String> {
    let mut schedule = salah::Schedule::<Local>::now();
    let mut s = &mut schedule;
    if let Some(parameters) = parameters {
        s = s
            .with_coordinates(Coordinates::new(parameters.latitude, parameters.longitude))
            .with_parameters(
                salah::Parameters::from_method(salah::Method::MuslimWorldLeague).with_madhab(parameters.madhab.into()),
            );
    }
    s.build()
}

#[post("/times/month/<month>", data = "<parameters>", rank = 2)]
fn new_monthly_timetable_str(month: &str, parameters: Option<Form<Parameters>>) -> Result<DayResponse<Local>, String> {
    let mut schedule = salah::Schedule::<Local>::now();
    let mut s = &mut schedule;
    if let Some(parameters) = parameters {
        s = s
            .with_coordinates(Coordinates::new(parameters.latitude, parameters.longitude))
            .with_parameters(
                salah::Parameters::from_method(salah::Method::MuslimWorldLeague).with_madhab(parameters.madhab.into()),
            );
    }
    s.build()
}
