use std::{
  collections::VecDeque,
  ffi::OsStr,
  fs::File,
  io::{self, Read},
  os::unix::prelude::{AsRawFd, FromRawFd},
  path::Path,
  process::{Child, Command, ExitStatus},
  time::Duration,
};

use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use log::trace;
use mio::{unix::pipe::Receiver, Events, Interest, Token};
use thiserror::Error;

use crate::PROGRESS;

const STDOUT: Token = Token(0);
const STDERR: Token = Token(1);

const BUFFER_SIZE: usize = 32;

#[derive(Error, Debug)]
pub enum ProcessError {
  #[error("io error while running script")]
  Io(#[from] std::io::Error),

  #[error("failed to read command output")]
  InvalidOutput(#[from] std::string::FromUtf8Error),

  #[error("process returned a non-zero exit code:\n{0}")]
  Failed(String),
}

lazy_static! {
  pub static ref PROGRESS_STYLE: ProgressStyle =
    ProgressStyle::with_template("{spinner:.cyan} {wide_msg}")
      .expect("failed to create progress style")
      .tick_strings(&[
        "⠋",
        "⠙",
        "⠹",
        "⠸",
        "⠼",
        "⠴",
        "⠦",
        "⠧",
        "⠇",
        "⠏",
        &style("✔").green().to_string()
      ]);
}

pub type Progress<'a> = (&'a ProgressBar, &'a str);

pub fn auto_bar() -> ProgressBar {
  let bar = PROGRESS.add(ProgressBar::new(1));
  bar.set_style(PROGRESS_STYLE.clone());
  bar.enable_steady_tick(Duration::from_millis(100));
  bar
}

pub fn run<Cmd, T>(
  show_progress: Option<Progress>,
  command: Cmd,
  args: Option<&[&str]>,
  dir: Option<&Path>,
  env: Option<&[(&str, &str)]>,
  parse_output: impl FnOnce(String) -> T + 'static,
) -> Result<T, ProcessError>
where
  Cmd: AsRef<OsStr>,
  T: 'static,
{
  let mut cmd = Command::new(command);

  if let Some(args) = args {
    cmd.args(args);
  }

  if let Some(path) = dir {
    cmd.current_dir(path);
  }

  if let Some(env) = env {
    trace!("Setting env for process:\n{env:#?}");
    for (key, val) in env {
      cmd.env(key, val);
    }
  }

  let (status, output_str, all_output) = if let Some((bar, message)) = show_progress {
    bar.set_message(message.to_string());
    let (status, output_str, all_output) = read_process(cmd, &bar, &message)?;
    bar.set_message(message.to_string());
    (status, output_str, all_output)
  } else {
    let output = cmd.output()?;
    let output_str = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status, output_str, stderr_str)
  };

  trace!("Got process output:\n{output_str}");

  if !status.success() {
    return Err(ProcessError::Failed(all_output));
  }

  Ok(parse_output(output_str))
}

fn read_process(
  cmd: Command,
  bar: &ProgressBar,
  message: &str,
) -> Result<(ExitStatus, String, String), io::Error> {
  let mut lines = Vec::new();
  let mut stdout_lines = Vec::new();
  let reader = ProcessReader::start(cmd)?;

  for line in reader {
    let line = match line {
      Ok(Out::Done(status)) => {
        let stdout = stdout_lines.join("\n");
        let all_output = lines.join("\n");
        return Ok((status, stdout, all_output));
      }
      Ok(Out::Stdout(line)) => {
        stdout_lines.push(line);
        continue;
      }
      Ok(Out::Stderr(line)) => line,
      Err(err) => return Err(err),
    };

    lines.push(line);
    let mut last_lines = lines
      .iter()
      .filter(|line| !line.is_empty())
      .rev()
      .take(3)
      .map(|line| format!("    {}", line))
      .collect::<Vec<_>>();

    last_lines.reverse();
    if last_lines.is_empty() {
      continue;
    }

    bar.set_message(format!(
      "{}\n{}",
      message.clone(),
      style(last_lines.join("\n")).dim()
    ));
  }

  unreachable!()
}

#[derive(Clone, Debug)]
enum Out {
  Stdout(String),
  Stderr(String),
  Done(ExitStatus),
}

#[derive(Clone, Copy, Debug)]
enum Stream {
  Stdout,
  Stderr,
}

struct ProcessReader {
  child: Child,

  stdout_read: Receiver,
  stderr_read: Receiver,

  stdout_buf: Vec<u8>,
  stderr_buf: Vec<u8>,
  output_buf: VecDeque<Out>,

  poll: mio::Poll,
  events: mio::Events,
  status: Option<ExitStatus>,
  done: bool,
}

impl ProcessReader {
  pub fn start(mut cmd: Command) -> Result<Self, io::Error> {
    let (stdout_write, mut stdout_read) = mio::unix::pipe::new()?;
    let (stderr_write, mut stderr_read) = mio::unix::pipe::new()?;

    let stdout_file = unsafe { File::from_raw_fd(stdout_write.as_raw_fd()) };
    let stderr_file = unsafe { File::from_raw_fd(stderr_write.as_raw_fd()) };

    let child = cmd.stdout(stdout_file).stderr(stderr_file).spawn()?;

    let poll = mio::Poll::new()?;
    let events = Events::with_capacity(128);

    poll
      .registry()
      .register(&mut stdout_read, STDOUT, Interest::READABLE)?;
    poll
      .registry()
      .register(&mut stderr_read, STDERR, Interest::READABLE)?;

    let stdout_buf = Vec::<u8>::new();
    let stderr_buf = Vec::<u8>::new();
    let output_buf = VecDeque::<Out>::new();

    Ok(Self {
      child,
      stdout_read,
      stderr_read,

      stdout_buf,
      stderr_buf,
      output_buf,

      poll,
      events,
      status: None,
      done: false,
    })
  }
}

fn read_pipe(
  reader: &mut Receiver,
  str_buf: &mut Vec<u8>,
  out_buf: &mut VecDeque<Out>,
  which: Stream,
) -> Result<(), io::Error> {
  loop {
    let mut buf = [0; BUFFER_SIZE];
    let n = match reader.read(&mut buf[..]) {
      Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
        return Ok(());
      }
      Ok(n) => Ok(n),
      err => err,
    }?;

    if n == 0 {
      if !str_buf.is_empty() {
        let line = String::from_utf8_lossy(&str_buf[..]).to_string();
        match which {
          Stream::Stdout => out_buf.push_back(Out::Stdout(line)),
          Stream::Stderr => out_buf.push_back(Out::Stderr(line)),
        };

        str_buf.clear();
      }

      return Ok(());
    }

    for i in 0..n {
      if buf[i] == b'\n' {
        let line = String::from_utf8_lossy(&str_buf[..]).to_string();
        match which {
          Stream::Stdout => out_buf.push_back(Out::Stdout(line)),
          Stream::Stderr => out_buf.push_back(Out::Stderr(line)),
        };

        str_buf.clear();
        continue;
      }

      if buf[i] == b'\r' {
        continue;
      }

      str_buf.push(buf[i]);
    }
  }
}

impl Iterator for ProcessReader {
  type Item = Result<Out, io::Error>;

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      if let Some(next) = self.output_buf.pop_front() {
        return Some(Ok(next));
      }

      if self.done {
        return None;
      }

      if let Some(status) = self.status {
        self.done = true;
        return Some(Ok(Out::Done(status)));
      }

      match self.child.try_wait() {
        Ok(None) => {}
        Ok(Some(status)) => {
          self.status = Some(status);
          continue;
        }
        Err(err) => return Some(Err(err)),
      };

      match self.poll.poll(&mut self.events, Some(Duration::from_millis(100))) {
        Err(err) => return Some(Err(err)),
        _ => {}
      };

      for event in self.events.iter() {
        match event.token() {
          STDOUT => match read_pipe(
            &mut self.stdout_read,
            &mut self.stdout_buf,
            &mut self.output_buf,
            Stream::Stdout,
          ) {
            Err(err) => return Some(Err(err)),
            _ => {}
          },
          STDERR => match read_pipe(
            &mut self.stderr_read,
            &mut self.stderr_buf,
            &mut self.output_buf,
            Stream::Stderr,
          ) {
            Err(err) => return Some(Err(err)),
            _ => {}
          },
          _ => unreachable!(),
        }
      }
    }
  }
}
