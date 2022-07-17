use std::{io::Write, time::Duration};

use clap::IntoApp;
use indicatif::ProgressBar;

use crate::Cli;

pub fn print_help_and_exit() -> ! {
  Cli::command().print_help().unwrap();

  // See https://github.com/clap-rs/clap/blob/a96e7cfc7fc155e86f4e08767b934bfcb666b665/src/util/mod.rs#L23
  let _ = std::io::stdout().lock().flush();
  let _ = std::io::stderr().lock().flush();
  std::process::exit(2);
}

pub fn auto_bar() -> ProgressBar {
  let bar = ProgressBar::new(1);
  bar.enable_steady_tick(Duration::from_millis(200));
  bar
}
