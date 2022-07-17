use std::{
  collections::{HashMap, HashSet},
  fs,
  io::{BufRead, BufReader},
  os::unix::prelude::PermissionsExt,
  path::{Path, PathBuf},
};

use console::style;
use flume::Receiver;
use indicatif::ProgressBar;
use lazy_static::lazy_static;
use log::trace;
use regex::Regex;
use thiserror::Error;
use threadpool::ThreadPool;

#[derive(Error, Debug)]
pub enum ProcessError {
  #[error("io error while running script")]
  Io(#[from] std::io::Error),

  #[error("failed to read command output")]
  InvalidOutput(#[from] std::string::FromUtf8Error),

  #[error("process returned a non-0 exit code:\n{0}")]
  Failed(String),
}

pub type BackgroundProcess<T> = Receiver<Result<T, ProcessError>>;

pub fn run_background<P: AsRef<Path>, T: 'static + Send>(
  bar: ProgressBar,
  pool: &ThreadPool,
  message: String,
  parse_output: fn(String) -> T,
  script: P,
  env: &[(&str, &str)],
) -> Result<BackgroundProcess<T>, ProcessError> {
  let (mut stderr_read, stderr_write) = os_pipe::pipe()?;
  let mut expr = duct::cmd!(script.as_ref())
    .stdout_capture()
    .stderr_file(stderr_write)
    .unchecked();

  trace!("Setting env for background process:\n{env:#?}");
  for (key, val) in env {
    expr = expr.env(key, val);
  }

  let (tx, rx) = flume::bounded(1);
  let reader = BufReader::new(stderr_read).lines();

  pool.execute(move || {
    let handle = match expr.start() {
      Ok(handle) => handle,
      Err(err) => {
        let _ = tx.send(Err(err.into()));
        return;
      }
    };

    let mut lines = Vec::new();
    for line in reader {
      let line = match line {
        Ok(line) => line,
        Err(_) => continue,
      };

      lines.push(line);
      let mut last_lines = lines
        .iter()
        .filter(|line| !line.is_empty())
        .rev()
        .take(3)
        .map(|line| format!("      {}", line))
        .collect::<Vec<_>>();

      last_lines.reverse();
      if last_lines.is_empty() {
        continue;
      }

      bar.set_message(format!(
        "{}\n{}",
        message,
        style(last_lines.join("\n")).dim()
      ));
    }

    let output = match handle.wait() {
      Ok(output) => output,
      Err(err) => {
        let _ = tx.send(Err(err.into()));
        return;
      }
    };

    let output_str = match String::from_utf8(output.stdout.clone()) {
      Ok(output_str) => output_str,
      Err(err) => {
        let _ = tx.send(Err(err.into()));
        return;
      }
    };

    trace!("Got background process output:\n{output_str}");
    if !output.status.success() {
      let _ = tx.send(Err(ProcessError::Failed(output_str)));
      return;
    }

    let parsed = parse_output(output_str);
    let _ = tx.send(Ok(parsed));
  });

  Ok(rx)
}

fn run<P: AsRef<Path>, T>(
  script: P,
  parse_output: fn(String) -> T,
  env: &[(&str, &str)],
) -> Result<T, ProcessError> {
  let mut expr = duct::cmd!(script.as_ref())
    .stderr_capture()
    .stdout_capture()
    .unchecked();

  trace!("Setting env for process:\n{env:#?}");

  for (key, val) in env {
    expr = expr.env(key, val);
  }

  let output = expr.run()?;
  let output_str = String::from_utf8(output.stdout)?;
  trace!("Got process output:\n{output_str}");

  if !output.status.success() {
    return Err(ProcessError::Failed(output_str));
  }

  Ok(parse_output(output_str))
}
