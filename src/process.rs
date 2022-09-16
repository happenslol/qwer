use std::{
  collections::{HashMap, HashSet},
  ffi::OsString,
  fs,
  io::{BufRead, BufReader},
  os::unix::prelude::PermissionsExt,
  path::{Path, PathBuf},
};

use console::style;
use flume::Receiver;
use indicatif::ProgressBar;
use lazy_static::lazy_static;
use log::{info, trace};
use regex::Regex;
use thiserror::Error;
use threadpool::ThreadPool;

use crate::cmds::util::auto_bar;

#[derive(Error, Debug)]
pub enum ProcessError {
  #[error("io error while running script")]
  Io(#[from] std::io::Error),

  #[error("failed to read command output")]
  InvalidOutput(#[from] std::string::FromUtf8Error),

  #[error("process returned a non-zero exit code:\n{0}")]
  Failed(String),
}

pub type BackgroundProcess<T> = Receiver<Result<T, ProcessError>>;

pub fn run_background<Cmd, T>(
  pool: &ThreadPool,
  message: String,
  command: Cmd,
  args: Option<&[&str]>,
  dir: Option<&Path>,
  env: Option<&[(&str, &str)]>,
  parse_output: impl FnOnce(String) -> T + Send + 'static,
) -> Result<BackgroundProcess<T>, ProcessError>
where
  Cmd: Into<OsString> + duct::IntoExecutablePath,
  T: 'static + Send,
{
  let mut expr = if let Some(args) = args {
    duct::cmd(command, args)
  } else {
    duct::cmd!(command)
  };

  expr = expr.stdout_capture().unchecked();

  if let Some(path) = dir {
    expr = expr.dir(path);
  }

  if let Some(env) = env {
    trace!("Setting env for background process:\n{env:#?}");
    for (key, val) in env {
      expr = expr.env(key, val);
    }
  }

  let (tx, rx) = flume::bounded(1);
  let bar = auto_bar();

  let (mut stderr_read, stderr_write) = os_pipe::pipe()?;

  pool.execute(move || {
    // This moves stderr_write into the temporary duct::Expression that drops at the end of
    // this statement. That's important; retaining it would deadlock the read loop below.
    let handle = match expr.stderr_file(stderr_write).start() {
      Ok(handle) => handle,
      Err(err) => {
        let _ = tx.send(Err(err.into()));
        return;
      }
    };

    let reader = BufReader::new(stderr_read).lines();
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
      let stderr_output = lines.join("\n");
      let mut combined = String::new();
      if !output_str.is_empty() {
        combined.push_str(&output_str);
      }

      if !stderr_output.is_empty() {
        if !combined.is_empty() {
          combined.push('\n');
        }

        combined.push_str(&stderr_output);
      }

      let _ = tx.send(Err(ProcessError::Failed(combined)));
      return;
    }

    let parsed = parse_output(output_str);
    let _ = tx.send(Ok(parsed));
  });

  Ok(rx)
}

pub fn run_foreground<Cmd, T>(
  command: Cmd,
  args: Option<&[&str]>,
  dir: Option<&Path>,
  env: Option<&[(&str, &str)]>,
  parse_output: impl FnOnce(String) -> T,
) -> Result<T, ProcessError>
where
  Cmd: Into<OsString> + duct::IntoExecutablePath,
{
  let mut expr = if let Some(args) = args {
    duct::cmd(command, args)
  } else {
    duct::cmd!(command)
  };

  expr = expr.stderr_capture().stdout_capture().unchecked();

  if let Some(path) = dir {
    expr = expr.dir(path);
  }

  if let Some(env) = env {
    trace!("Setting env for background process:\n{env:#?}");
    for (key, val) in env {
      expr = expr.env(key, val);
    }
  }

  let output = expr.run()?;
  let output_str = String::from_utf8(output.stdout)?;
  trace!("Got process output:\n{output_str}");

  if !output.status.success() {
    return Err(ProcessError::Failed(output_str));
  }

  Ok(parse_output(output_str))
}
