use std::io::Write;

use clap::IntoApp;

use crate::Cli;

pub fn print_help_and_exit() -> ! {
    Cli::command().print_help().unwrap();

    // See https://github.com/clap-rs/clap/blob/a96e7cfc7fc155e86f4e08767b934bfcb666b665/src/util/mod.rs#L23
    let _ = std::io::stdout().lock().flush();
    let _ = std::io::stderr().lock().flush();
    std::process::exit(2);
}
