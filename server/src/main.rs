use rocket::{launch, routes};

use salah_server::server::{
    index, new_current_month_timetable, new_current_timetable, new_daily_timetable, new_monthly_timetable,
    new_monthly_timetable_short_str, new_monthly_timetable_str,
};

#[launch]
fn rocket() -> _ {
    rocket::build().mount(
        "/",
        routes![
            index,
            new_current_timetable,
            new_daily_timetable,
            new_current_month_timetable,
            new_monthly_timetable,
            new_monthly_timetable_short_str,
            new_monthly_timetable_str
        ],
    )
}
