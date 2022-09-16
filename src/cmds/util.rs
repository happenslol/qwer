use std::{io::Write, time::Duration};

use clap::IntoApp;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;

use crate::{Cli, PROGRESS};

lazy_static! {
  pub static ref PROGRESS_STYLE: ProgressStyle =
    ProgressStyle::with_template("  {spinner} {wide_msg}")
      .expect("failed to create progress style")
      .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");
  pub static ref STATUS_STYLE: ProgressStyle =
    ProgressStyle::with_template("  {prefix} {wide_msg}").expect("failed to create status style");
  pub static ref DONE_STYLE: ProgressStyle =
    ProgressStyle::with_template("  {prefix} {wide_msg}").expect("failed to create done style");
}

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
  PROGRESS.add(bar.clone());

  bar
}
