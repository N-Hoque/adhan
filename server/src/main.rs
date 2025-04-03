use std::net::{IpAddr, Ipv4Addr};

use rocket::{launch, routes, Config};

use salah_server::server::routes::{
    index, times_month, times_now, times_day, times_month_month,
    times_month_month_str,
};

#[launch]
fn rocket() -> _ {
    let profile = std::env::var("PROFILE").expect("PROFILE environment variable is set");

    let mut config = if profile == "DEBUG" {
        Config::debug_default()
    } else {
        Config::release_default()
    };
    config.address = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));

    rocket::build().configure(config).mount(
        "/",
        routes![
            index,
            times_now,
            times_day,
            times_month,
            times_month_month,
            times_month_month_str,
        ],
    )
}
