use embed_manifest::{embed_manifest, new_manifest};
use time::{macros::format_description, OffsetDateTime};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Embed an application manifest declaring requestedExecutionLevel=asInvoker.
    // Without a manifest, Windows UAC "Installer Detection" forces an elevation
    // prompt (os error 740) for any exe whose name contains keywords like
    // "update"/"setup"/"install" — which broke the self-update temp copy.
    // Setting an explicit execution level disables that heuristic for all our
    // binaries regardless of filename.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("Mdrcp")).expect("failed to embed application manifest");
    }

    let now = OffsetDateTime::now_utc();
    let format = format_description!("[year]-[month]-[day] [hour]:[minute]:[second] UTC");
    let formatted = now
        .format(format)
        .expect("failed to format build timestamp");

    println!("cargo:rustc-env=MD_BUILD_TIMESTAMP={}", formatted);
}
