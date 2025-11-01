use std::{env, path::Path, process};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match mdrcp::parse_args(&args) {
        Ok(mdrcp::Command::ShowHelp) => {
            mdrcp::print_help();
            process::exit(0);
        }
        Ok(mdrcp::Command::ShowVersion) => {
            mdrcp::print_version_banner();
            process::exit(0);
        }
        Ok(mdrcp::Command::Deploy(options)) => {
            if !options.quiet {
                mdrcp::print_version_banner();
            }
            process::exit(mdrcp::do_main_with_options(Path::new("."), &options));
        }
        Err(err) => {
            mdrcp::print_parse_error(&err);
            process::exit(1);
        }
    }
}
