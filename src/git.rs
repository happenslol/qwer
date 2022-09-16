use std::path::{Path, PathBuf};

use console::style;
use indicatif::{MultiProgress, ProgressBar};
use log::trace;
use thiserror::Error;
use threadpool::ThreadPool;

use crate::{
  process::{run_background, run_foreground, BackgroundProcess, ProcessError},
  PROGRESS,
};

#[derive(Error, Debug)]
pub enum GitError {
  #[error("io error while running git command")]
  Io(#[from] std::io::Error),

  #[error("git command returned an error:\n{0}")]
  Command(String),

  #[error("failed to read command output")]
  Output(#[from] std::string::FromUtf8Error),

  #[error("`{0}` is not a git directory")]
  NotAGitDirectory(PathBuf),

  #[error("error while running git command: {0}")]
  ProcessError(#[from] ProcessError),

  #[error("failed to receive background task result: {0}")]
  BackgroundError(#[from] flume::RecvError),
}

pub struct GitRepo {
  git_dir: PathBuf,
  work_tree: PathBuf,
}

impl GitRepo {
  pub fn new<P: AsRef<Path>>(dir: P) -> Result<Self, GitError> {
    let work_tree = PathBuf::from(dir.as_ref());
    let git_dir = dir.as_ref().join(".git");
    if !git_dir.is_dir() {
      return Err(GitError::NotAGitDirectory(git_dir));
    }

    trace!("Initialized git repo at {:?}", work_tree);
    Ok(Self { git_dir, work_tree })
  }

  pub fn clone<P: AsRef<Path>>(
    pool: &ThreadPool,
    dir: P,
    url: &str,
    name: &str,
    branch: Option<&str>,
  ) -> Result<Self, GitError> {
    trace!(
      "Cloning repo `{}@{:?}` into {:?}",
      url,
      branch,
      dir.as_ref()
    );

    let mut args = vec!["clone", url, name];
    if let Some(branch) = branch {
      args.push(branch);
    }

    run_background(
      pool,
      format!("Cloning {name}"),
      "git",
      Some(&args),
      Some(dir.as_ref()),
      None,
      |output| output,
    )?
    .recv()??;

    let work_tree = dir.as_ref().join(name);
    let git_dir = work_tree.join(".git");

    Ok(Self { git_dir, work_tree })
  }

  fn git_background<T>(
    &self,
    pool: &ThreadPool,
    message: String,
    args: &[&str],
    parse_output: impl FnOnce(String) -> T + Send + 'static,
  ) -> Result<BackgroundProcess<T>, GitError>
  where
    T: 'static + Send,
  {
    let git_dir_str = self.git_dir.to_string_lossy();
    let work_tree_str = self.work_tree.to_string_lossy();

    let args_with_dirs = &[
      &["--git-dir", &git_dir_str, "--work-tree", &work_tree_str],
      args,
    ]
    .concat();

    if log::log_enabled!(log::Level::Trace) {
      let args_str = args_with_dirs.join(" ");
      trace!("Running git command `{args_str}`");
    }

    Ok(run_background(
      pool,
      message,
      "git",
      Some(args_with_dirs),
      Some(&self.git_dir),
      None,
      parse_output,
    )?)
  }

  fn git_foreground<T>(
    &self,
    args: &[&str],
    parse_output: impl FnOnce(String) -> T,
  ) -> Result<T, GitError> {
    let git_dir_str = self.git_dir.to_string_lossy();
    let work_tree_str = self.work_tree.to_string_lossy();

    let args_with_dirs = &[
      &["--git-dir", &git_dir_str, "--work-tree", &work_tree_str],
      args,
    ]
    .concat();

    if log::log_enabled!(log::Level::Trace) {
      let args_str = args_with_dirs.join(" ");
      trace!("Running git command `{args_str}`");
    }

    Ok(run_foreground(
      "git",
      Some(args_with_dirs),
      Some(&self.git_dir),
      None,
      parse_output,
    )?)
  }

  fn find_remote_default_branch(
    &self,
    pool: &ThreadPool,
  ) -> Result<BackgroundProcess<String>, GitError> {
    Ok(self.git_background(
      pool,
      format!("Resolving remote default branch"),
      &["remote", "show", "origin"],
      |output| {
        output
          .split('\n')
          .find(|line| line.trim().starts_with("HEAD branch:"))
          // Default to main if nothing is found
          .unwrap_or("main")
          .trim()
          .trim_start_matches("HEAD branch:")
          .trim()
          .to_string()
      },
    )?)
  }

  fn force_checkout(&self, rref: &str) -> Result<(), GitError> {
    self.git_foreground(
      &[
        "-c",
        "advice.detachedHead=false",
        "checkout",
        rref,
        "--force",
      ],
      |_| (),
    )?;

    Ok(())
  }

  pub fn get_remote_url(&self) -> Result<String, GitError> {
    self.git_foreground(&["remote", "get-url", "origin"], |output| output)
  }

  pub fn get_head_branch(&self) -> Result<String, GitError> {
    self.git_foreground(&["rev-parse", "--abbrev-ref", "HEAD"], |output| output)
  }

  pub fn get_head_ref(&self) -> Result<String, GitError> {
    self.git_foreground(&["rev-parse", "--short", "HEAD"], |output| output)
  }

  pub fn update_to_ref(&self, pool: &ThreadPool, rref: &str) -> Result<(), GitError> {
    self
      .git_background(
        pool,
        format!("Fetching {rref}"),
        &["fetch", "--prune", "origin"],
        |_| (),
      )?
      .recv()??;

    self.force_checkout(rref)?;
    Ok(())
  }

  pub fn update_to_remote_head(&self, pool: &ThreadPool) -> Result<(), GitError> {
    let remote_default_branch = self.find_remote_default_branch(pool)?.recv()??;

    trace!("Fetching from remote default branch `{remote_default_branch}`");
    let fetch_arg = format!("{remote_default_branch}:{remote_default_branch}");
    self
      .git_background(
        pool,
        format!("Fetching {remote_default_branch}"),
        &["fetch", "--prune", "--update-head-ok", "origin", &fetch_arg],
        |_| (),
      )?
      .recv()??;

    trace!("Resetting to origin/{remote_default_branch}");
    let remote_ref = format!("origin/{remote_default_branch}");
    self.git_foreground(&["reset", "--hard", &remote_ref], |_| ())?;

    Ok(())
  }
}
