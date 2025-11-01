use time::{macros::format_description, OffsetDateTime};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let now = OffsetDateTime::now_utc();
    let format = format_description!("[year]-[month]-[day] [hour]:[minute]:[second] UTC");
    let formatted = now
        .format(format)
        .expect("failed to format build timestamp");

    println!("cargo:rustc-env=MD_BUILD_TIMESTAMP={}", formatted);
}
