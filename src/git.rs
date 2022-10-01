use std::path::{Path, PathBuf};

use console::style;
use indicatif::ProgressBar;
use log::trace;
use thiserror::Error;

use crate::process::{auto_bar, run, ProcessError, Progress};

#[derive(Error, Debug)]
pub enum GitError {
  #[error("IO error while running git command")]
  Io(#[from] std::io::Error),

  #[error("Failed to read command output")]
  Output(#[from] std::string::FromUtf8Error),

  #[error("{0} is not a git directory")]
  NotAGitDirectory(PathBuf),

  #[error("Error while running git command: {0}")]
  ProcessError(#[from] ProcessError),
}

#[derive(Debug, Clone)]
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
    progress: Progress,
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

    run(
      Some(progress),
      "git",
      Some(&args),
      Some(dir.as_ref()),
      None,
      |output| output,
    )?;

    let work_tree = dir.as_ref().join(name);
    let git_dir = work_tree.join(".git");

    Ok(Self { git_dir, work_tree })
  }

  fn run_git<T>(
    &self,
    bar: Option<(&ProgressBar, &str)>,
    args: &[&str],
    parse_output: impl FnOnce(String) -> T + 'static,
  ) -> Result<T, GitError>
  where
    T: 'static,
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

    Ok(run(
      bar,
      "git",
      Some(args_with_dirs),
      Some(&self.git_dir),
      None,
      parse_output,
    )?)
  }

  fn find_remote_default_branch(&self, progress: Progress) -> Result<String, GitError> {
    self.run_git(Some(progress), &["remote", "show", "origin"], |output| {
      output
        .split('\n')
        .find(|line| line.trim().starts_with("HEAD branch:"))
        // Default to main if nothing is found
        .unwrap_or("main")
        .trim()
        .trim_start_matches("HEAD branch:")
        .trim()
        .to_string()
    })
  }

  fn force_checkout(&self, rref: &str) -> Result<(), GitError> {
    self.run_git(
      None,
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
    self.run_git(None, &["remote", "get-url", "origin"], |output| output)
  }

  pub fn get_head_branch(&self) -> Result<String, GitError> {
    self.run_git(None, &["rev-parse", "--abbrev-ref", "HEAD"], |output| {
      output
    })
  }

  pub fn get_head_ref(&self) -> Result<String, GitError> {
    self.run_git(None, &["rev-parse", "--short", "HEAD"], |output| output)
  }

  pub fn update_to_ref(&self, progress: Progress, rref: &str) -> Result<(), GitError> {
    self.run_git(Some(progress), &["fetch", "--prune", "origin"], |_| ())?;
    self.force_checkout(rref)?;
    Ok(())
  }

  pub fn update_to_remote_head(
    &self,
    find_head_message: Option<&str>,
    fetch_head_message: Option<&str>,
  ) -> Result<(), GitError> {
    let bar = auto_bar();
    let find_head_message = find_head_message
      .map(|it| it.to_string())
      .unwrap_or_else(|| String::from("Finding remote head branch"));

    let remote_default_branch = self.find_remote_default_branch((&bar, &find_head_message))?;

    let fetch_head_message = fetch_head_message
      .map(|it| it.to_string())
      .unwrap_or_else(|| {
        format!(
          "Fetching remote branch {}",
          style(&remote_default_branch).bold()
        )
      });

    trace!("Fetching from remote default branch `{remote_default_branch}`");
    let fetch_arg = format!("{remote_default_branch}:{remote_default_branch}");
    self.run_git(
      Some((&bar, &fetch_head_message)),
      &["fetch", "--prune", "--update-head-ok", "origin", &fetch_arg],
      |_| (),
    )?;

    trace!("Resetting to origin/{remote_default_branch}");
    let remote_ref = format!("origin/{remote_default_branch}");
    self.run_git(None, &["reset", "--hard", &remote_ref], |_| ())?;

    Ok(())
  }
}
