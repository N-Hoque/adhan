use std::collections::{BTreeMap, HashMap};

use chrono::{Date, NaiveDate};
use rocket::{
    form::{Form, Strict},
    get, post,
    serde::json::Json,
    time::util::is_leap_year,
    FromForm, FromFormField,
};
use salah::{Coordinates, DateTime, Datelike, TimeZone, Times, Utc};
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
    latitude: Strict<f64>,
    longitude: Strict<f64>,

    madhab: Madhab,
}

#[derive(Clone)]
struct Parameters {
    latitude: f64,
    longitude: f64,

    madhab: Madhab,
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
    fajr: chrono::NaiveTime,
    dhuhr: chrono::NaiveTime,
    asr: chrono::NaiveTime,
    maghrib: chrono::NaiveTime,
    isha: chrono::NaiveTime,
}

#[derive(Serialize)]
pub struct DayResponse {
    timetable: Timetable,
    current_date: NaiveDate,
    current_event: salah::Event,
}

#[derive(Serialize)]
pub struct MonthResponse {
    days: BTreeMap<NaiveDate, Timetable>,
    current_date: NaiveDate,
    total_days: u8,
}

#[get("/")]
pub fn index() -> &'static str {
    "Welcome to Adhan!"
}

fn new_timetable_by_day(day: u8, parameters: Option<Parameters>) -> Result<Times<Utc>, String> {
    let current_date = chrono::Utc::now();
    let new_date = current_date
        .with_day(day as u32)
        .ok_or_else(|| format!("invalid day {} for month {}", day, current_date.month()))?;
    let mut schedule = salah::Schedule::new();
    let mut schedule = schedule.with_date(&new_date);
    if let Some(parameters) = parameters {
        schedule = schedule
            .with_coordinates(Coordinates::new(parameters.latitude, parameters.longitude))
            .with_parameters(
                salah::Parameters::from_method(salah::Method::MuslimWorldLeague).with_madhab(parameters.madhab.into()),
            );
    }
    schedule.build()
}

#[post("/times/now", data = "<parameters>")]
pub fn new_current_timetable(parameters: Option<Form<FormParameters>>) -> Result<Json<DayResponse>, String> {
    println!("{:?}", parameters);
    let mut schedule = salah::Schedule::<Utc>::now();
    let mut s = &mut schedule;
    if let Some(parameters) = parameters {
        s = s
            .with_coordinates(Coordinates::new(*parameters.latitude, *parameters.longitude))
            .with_parameters(
                salah::Parameters::from_method(salah::Method::MuslimWorldLeague).with_madhab(parameters.madhab.into()),
            );
    }
    s.build().map(|t| {
        Json(DayResponse {
            timetable: map_times_to_timetable(&t),
            current_date: t.asr().date_naive(),
            current_event: t.expected(&Utc::now()).current_event(),
        })
    })
}

#[post("/times/<day>", data = "<parameters>")]
pub fn new_daily_timetable(day: u8, parameters: Option<Form<FormParameters>>) -> Result<Json<DayResponse>, String> {
    let mut real_parameters = None;
    if let Some(parameters) = parameters {
        real_parameters = Some(Parameters::from(parameters));
    };

    new_timetable_by_day(day, real_parameters).map(|t| {
        Json(DayResponse {
            timetable: map_times_to_timetable(&t),
            current_date: t.asr().date_naive(),
            current_event: t.expected(&Utc::now()).current_event(),
        })
    })
}

#[post("/times/month", data = "<parameters>")]
pub fn new_current_month_timetable(parameters: Option<Form<FormParameters>>) -> Result<Json<MonthResponse>, String> {
    let current_date = Utc::now();
    let max_days = match current_date.month() {
        2 if is_leap_year(current_date.year()) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };

    let mut real_parameters = None;
    if let Some(parameters) = parameters {
        real_parameters = Some(Parameters::from(parameters));
    };

    let days = (1..=max_days)
        .map(|d| new_timetable_by_day(d, real_parameters.clone()))
        .collect::<Result<Vec<Times<Utc>>, String>>();

    days.map(|days| {
        Json(MonthResponse {
            days: {
                let mut m = BTreeMap::new();
                for d in &days {
                    m.entry(d.asr().date_naive())
                        .or_insert_with(|| map_times_to_timetable(d));
                }
                m
            },
            current_date: Utc::now().date_naive(),
            total_days: max_days,
        })
    })
}

#[post("/times/month/<month>", data = "<parameters>")]
pub fn new_monthly_timetable(
    month: u8,
    parameters: Option<Form<FormParameters>>,
) -> Result<Json<Vec<Timetable>>, String> {
    let current_date = Utc::now()
        .with_month(month as u32)
        .ok_or_else(|| format!("month should be between 1-12, got {}", month))?;
    let max_days = match current_date.month() {
        2 if is_leap_year(current_date.year()) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };

    let mut real_parameters = None;
    if let Some(parameters) = parameters {
        real_parameters = Some(Parameters::from(parameters));
    };

    let days = (1..=max_days)
        .map(|d| new_timetable_by_day(d, real_parameters.clone()))
        .collect::<Result<Vec<Times<Utc>>, String>>();

    days.map(|days| {
        Json(
            days.iter()
                .map(|day| Timetable {
                    fajr: day.fajr().time(),
                    dhuhr: day.dhuhr().time(),
                    asr: day.asr().time(),
                    maghrib: day.maghrib().time(),
                    isha: day.isha().time(),
                })
                .collect(),
        )
    })
}

#[post("/times/month/<month>", data = "<parameters>", rank = 2)]
pub fn new_monthly_timetable_short_str(
    month: &str,
    parameters: Option<Form<FormParameters>>,
) -> Result<Json<Vec<Timetable>>, String> {
    let month = short_month_to_int(month)?;

    let current_date = Utc::now()
        .with_month(month as u32)
        .ok_or_else(|| format!("month should be between 1-12, got {}", month))?;
    let max_days = match current_date.month() {
        2 if is_leap_year(current_date.year()) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };

    let mut real_parameters = None;
    if let Some(parameters) = parameters {
        real_parameters = Some(Parameters::from(parameters));
    };

    let days = (1..=max_days)
        .map(|d| new_timetable_by_day(d, real_parameters.clone()))
        .collect::<Result<Vec<Times<Utc>>, String>>();

    days.map(|days| {
        Json(
            days.iter()
                .map(|day| Timetable {
                    fajr: day.fajr().time(),
                    dhuhr: day.dhuhr().time(),
                    asr: day.asr().time(),
                    maghrib: day.maghrib().time(),
                    isha: day.isha().time(),
                })
                .collect(),
        )
    })
}

#[post("/times/month/<month>", data = "<parameters>", rank = 3)]
pub fn new_monthly_timetable_str(
    month: &str,
    parameters: Option<Form<FormParameters>>,
) -> Result<Json<Vec<Timetable>>, String> {
    let month = short_month_to_int(month)?;

    let current_date = Utc::now()
        .with_month(month as u32)
        .ok_or_else(|| format!("month should be between 1-12, got {}", month))?;
    let max_days = match current_date.month() {
        2 if is_leap_year(current_date.year()) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };

    let mut real_parameters = None;
    if let Some(parameters) = parameters {
        real_parameters = Some(Parameters::from(parameters));
    };

    let days = (1..=max_days)
        .map(|d| new_timetable_by_day(d, real_parameters.clone()))
        .collect::<Result<Vec<Times<Utc>>, String>>();

    days.map(|days| {
        Json(
            days.iter()
                .map(|day| Timetable {
                    fajr: day.fajr().time(),
                    dhuhr: day.dhuhr().time(),
                    asr: day.asr().time(),
                    maghrib: day.maghrib().time(),
                    isha: day.isha().time(),
                })
                .collect(),
        )
    })
}

fn map_times_to_timetable<Tz: TimeZone>(times: &Times<Tz>) -> Timetable {
    Timetable {
        fajr: times.fajr().time(),
        dhuhr: times.dhuhr().time(),
        asr: times.asr().time(),
        maghrib: times.maghrib().time(),
        isha: times.isha().time(),
    }
}

fn short_month_to_int(month: &str) -> Result<u8, String> {
    let month = match month.to_lowercase().as_str() {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return Err(format!("invalid short month name: {}", month)),
    };

    Ok(month)
}

fn month_to_int(month: &str) -> Result<u8, String> {
    let month = match month.to_lowercase().as_str() {
        "january" => 1,
        "february" => 2,
        "march" => 3,
        "april" => 4,
        "may" => 5,
        "june" => 6,
        "july" => 7,
        "august" => 8,
        "september" => 9,
        "october" => 10,
        "november" => 11,
        "december" => 12,
        _ => return Err(format!("invalid month name: {}", month)),
    };

    Ok(month)
}

fn month_num_to_name(month: u8) -> &'static str {
    match month {
        1 => "January",
        2 => "Feburary",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => unreachable!(),
    }
}
