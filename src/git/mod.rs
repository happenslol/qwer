use std::path::{Path, PathBuf};

use console::style;
use log::{info, trace};
use thiserror::Error;

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

        info!("Cloning {url}");
        run("git", &dir, &args)?;
        let work_tree = dir.as_ref().join(name);
        let git_dir = work_tree.join(".git");

        Ok(Self { git_dir, work_tree })
    }

    fn run(&self, args: &[&str]) -> Result<String, GitError> {
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

        let output = run("git", &self.git_dir, args_with_dirs)?.trim().to_owned();
        trace!("git command output:\n{output}");

        Ok(output)
    }

    fn find_remote_default_branch(&self) -> Result<String, GitError> {
        let output = self.run(&["ls-remote", "--symref", "origin", "HEAD"])?;

        let result = output.split('\n').collect::<Vec<&str>>()[0]
            .trim_start_matches("ref: refs/heads/")
            .trim_end_matches("HEAD")
            .trim()
            .to_owned();

        trace!("parsed remote default branch as `{result}`");

        Ok(result)
    }

    fn force_checkout(&self, rref: &str) -> Result<(), GitError> {
        info!("Checking out {}", style(rref).blue());

        self.run(&[
            "-c",
            "advice.detachedHead=false",
            "checkout",
            rref,
            "--force",
        ])?;

        Ok(())
    }

    pub fn get_remote_url(&self) -> Result<String, GitError> {
        self.run(&["remote", "get-url", "origin"])
    }

    pub fn get_head_branch(&self) -> Result<String, GitError> {
        self.run(&["rev-parse", "--abbrev-ref", "HEAD"])
    }

    pub fn get_head_ref(&self) -> Result<String, GitError> {
        self.run(&["rev-parse", "--short", "HEAD"])
    }

    pub fn update_to_ref(&self, rref: &str) -> Result<(), GitError> {
        self.run(&["fetch", "--prune", "origin"])?;
        self.force_checkout(rref)?;
        Ok(())
    }

    pub fn update_to_remote_head(&self) -> Result<(), GitError> {
        info!("Updating to latest remote");
        let remote_default_branch = self.find_remote_default_branch()?;

        trace!("Fetching from remote default branch `{remote_default_branch}`");
        let fetch_arg = format!("{remote_default_branch}:{remote_default_branch}");
        self.run(&["fetch", "--prune", "--update-head-ok", "origin", &fetch_arg])?;

        trace!("Resetting to origin/{remote_default_branch}");
        let remote_ref = format!("origin/{remote_default_branch}");
        self.run(&["reset", "--hard", &remote_ref])?;

        Ok(())
    }
}

fn run<P: AsRef<Path>>(cmd: &str, dir: P, args: &[&str]) -> Result<String, GitError> {
    let output = duct::cmd(cmd, args)
        .dir(dir.as_ref())
        .stderr_to_stdout()
        .stdout_capture()
        .unchecked()
        .run()?;

    let output_str = String::from_utf8(output.stdout)?;
    if !output.status.success() {
        return Err(GitError::Command(output_str));
    }

    Ok(output_str)
}
