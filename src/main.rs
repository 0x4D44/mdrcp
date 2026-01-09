use std::{env, path::Path, process};

const UPDATER_TEMP_NAME: &str = "mdrcp_updater.exe";

fn main() {
    // Clean up any leftover temp updater from a previous self-update
    cleanup_old_updater();

    let args: Vec<String> = env::args().skip(1).collect();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();

    match mdrcp::parse_args(&args) {
        Ok(mdrcp::Command::ShowHelp) => {
            let _ = mdrcp::write_help(&mut stdout);
            process::exit(0);
        }
        Ok(mdrcp::Command::ShowVersion) => {
            let _ = mdrcp::write_version_banner(&mut stdout);
            process::exit(0);
        }
        Ok(mdrcp::Command::Deploy(options)) => {
            if !options.quiet {
                let _ = mdrcp::write_version_banner(&mut stdout);
            }
            process::exit(mdrcp::do_main_with_options(Path::new("."), &options));
        }
        Ok(mdrcp::Command::FinishUpdate { source, dest }) => {
            process::exit(finish_update(&source, &dest));
        }
        Err(err) => {
            let _ = mdrcp::write_parse_error(&mut stderr, &err);
            process::exit(1);
        }
    }
}

/// Clean up any leftover temp updater executable from a previous self-update.
fn cleanup_old_updater() {
    let temp_dir = env::temp_dir();
    let updater_path = temp_dir.join(UPDATER_TEMP_NAME);
    if updater_path.exists() {
        // Best effort - ignore errors (file might be in use by another process)
        let _ = std::fs::remove_file(&updater_path);
    }
}

/// Perform the actual copy for a self-update, with retries.
/// Called by the temp updater executable.
fn finish_update(source: &Path, dest: &Path) -> i32 {
    use std::{thread, time::Duration};

    const MAX_RETRIES: u32 = 10;
    const RETRY_DELAY_MS: u64 = 100;

    for attempt in 1..=MAX_RETRIES {
        match std::fs::copy(source, dest) {
            Ok(_) => {
                eprintln!("Self-update complete: {}", dest.display());
                return 0;
            }
            Err(e) => {
                if attempt < MAX_RETRIES {
                    thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
                } else {
                    eprintln!("Self-update failed after {} attempts: {}", MAX_RETRIES, e);
                    return 1;
                }
            }
        }
    }
    1
}
