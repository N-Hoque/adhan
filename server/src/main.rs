use std::net::{IpAddr, Ipv4Addr};

use rocket::{launch, routes, Config};

use salah_server::server::routes::{
    index, times_day, times_month, times_month_month, times_month_month_str, times_now, times_today,
};

#[launch]
fn rocket() -> _ {
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| String::from("DEBUG"));

    let default_config = if profile == "DEBUG" {
        Config::debug_default()
    } else {
        Config::release_default()
    };

    let config = Config {
        address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        ..default_config
    };

    rocket::build().configure(config).mount(
        "/",
        routes![
            index,
            times_now,
            times_today,
            times_day,
            times_month,
            times_month_month,
            times_month_month_str,
        ],
    )
}
