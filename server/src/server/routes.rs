use std::collections::BTreeMap;

use rocket::{form::Form, get, post, response::Redirect, serde::json::Json, time::util::is_leap_year, uri};
use salah::{Coordinates, Datelike, TimeZone, Times, Utc};

use crate::server::model::DayResponse;

use super::model::{FormParameters, MonthResponse, Parameters, Timetable};

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
pub fn times_now(parameters: Option<Form<FormParameters>>) -> Result<Json<DayResponse>, String> {
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

#[post("/times/today", data = "<parameters>")]
pub fn times_today(parameters: Option<Form<FormParameters>>) -> Redirect {
    Redirect::permanent(uri!(times_now))
}

#[post("/times/day/<day>", data = "<parameters>")]
pub fn times_day(day: u8, parameters: Option<Form<FormParameters>>) -> Result<Json<DayResponse>, String> {
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
pub fn times_month(parameters: Option<Form<FormParameters>>) -> Result<Json<MonthResponse>, String> {
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
pub fn times_month_month(
    month: u8,
    parameters: Option<Form<FormParameters>>,
) -> Result<Json<MonthResponse>, String> {
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

#[post("/times/month/<month>", data = "<parameters>", rank = 2)]
pub fn times_month_month_str(
    month: &str,
    parameters: Option<Form<FormParameters>>,
) -> Result<Json<MonthResponse>, String> {
    let long_month = month_to_int(month);
    let short_month = short_month_to_int(month);

    let month = match (long_month, short_month) {
        (Some(long), _) => long,
        (_, Some(short)) => short,
        (None, None) => return Err(format!("{} is not a valid month name", month)),
    };

    let current_date = Utc::now()
        .with_month(month as u32)
        .ok_or_else(|| format!("invalid short month name, got {}", month))?;
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

fn map_times_to_timetable<Tz: TimeZone>(times: &Times<Tz>) -> Timetable {
    Timetable {
        fajr: times.fajr().time(),
        dhuhr: times.dhuhr().time(),
        asr: times.asr().time(),
        maghrib: times.maghrib().time(),
        isha: times.isha().time(),
    }
}

fn short_month_to_int(month: &str) -> Option<u8> {
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
        _ => return None,
    };

    Some(month)
}

fn month_to_int(month: &str) -> Option<u8> {
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
        _ => return None,
    };

    Some(month)
}
