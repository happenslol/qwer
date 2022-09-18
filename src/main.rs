#![allow(unused)]
use std::{io::Write, path::Path};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use console::style;
use indicatif::MultiProgress;
use log::{error, trace};
use threadpool::ThreadPool;

use crate::{
  dirs::{get_dir, BIN_DIR},
  shell::Shell,
};

mod cmds;
mod dirs;
mod env;
mod git;
mod plugins;
mod process;
mod scripts;
mod shell;
mod versions;

#[derive(Debug, Parser)]
#[clap(name = "qwer", author, version, about)]
struct Cli {
  #[clap(subcommand)]
  command: Commands,
}

#[derive(Debug, Subcommand)]
#[clap(disable_help_subcommand(true), allow_external_subcommands(true))]
enum Commands {
  #[clap(hide = true)]
  Hook {
    #[clap(subcommand)]
    shell: ShellOptions,
  },

  #[clap(hide = true)]
  Export {
    #[clap(subcommand)]
    shell: ShellOptions,
  },

  Plugin {
    #[clap(subcommand)]
    command: PluginCommand,
  },

  Use {
    name: Option<String>,
    version: Option<String>,
  },

  Install {
    name: Option<String>,
    version: Option<String>,

    #[clap(long, short)]
    concurrency: Option<usize>,

    #[clap(long, short)]
    keep_download: bool,
  },

  Uninstall {
    name: String,
    version: String,
  },

  Current {
    name: String,
  },

  Where {
    name: String,
    version: Option<String>,
  },

  Latest {
    name: String,
    filter: Option<String>,
  },

  List {
    #[clap(subcommand)]
    command: Option<ListCommand>,

    name: Option<String>,
    filter: Option<String>,
  },

  Global {
    name: String,
    version: String,
  },

  Local {
    name: String,
    version: String,
  },

  Shell {
    name: String,
    version: String,
  },

  Help {
    plugin: Option<String>,
    version: Option<String>,
  },

  #[clap(hide = true)]
  Reshim {
    args: Vec<String>,
  },

  #[clap(hide = true)]
  Which {
    args: Vec<String>,
  },

  #[clap(external_subcommand)]
  Command(Vec<String>),
}

#[derive(Debug, Subcommand)]
enum ShellOptions {
  Bash,
  Zsh,
}

#[derive(Debug, Subcommand)]
enum PluginCommand {
  Add {
    name: String,
    git_url: Option<String>,
  },

  List {
    #[clap(subcommand)]
    command: Option<PluginListCommand>,

    #[clap(short, long)]
    urls: bool,

    #[clap(short, long)]
    refs: bool,
  },

  Remove {
    name: String,
  },

  Update {
    #[clap(subcommand)]
    command: Option<PluginUpdateCommand>,

    name: Option<String>,
    git_ref: Option<String>,
  },
}

#[derive(Debug, Subcommand)]
enum PluginListCommand {
  All,
}

#[derive(Debug, Subcommand)]
enum PluginUpdateCommand {
  All,
}

#[derive(Debug, Subcommand)]
enum ListCommand {
  All {
    name: String,
    filter: Option<String>,
  },
}

impl ShellOptions {
  fn get(&self) -> &dyn Shell {
    match self {
      ShellOptions::Bash => &shell::Bash,
      ShellOptions::Zsh => &shell::Zsh,
    }
  }

  fn name(&self) -> &'static str {
    match self {
      ShellOptions::Bash => "bash",
      ShellOptions::Zsh => "zsh",
    }
  }
}

fn ensure_asdf_alias(self_path: &Path) -> Result<()> {
  let asdf_bin = get_dir(BIN_DIR)?.join("asdf");
  if !asdf_bin.is_symlink() {
    std::os::unix::fs::symlink(self_path, asdf_bin)?;
  }

  Ok(())
}

fn assert_running_qwer(is_asdf: bool) -> Result<()> {
  if is_asdf {
    bail!("This command can not be run from an asdf symlink");
  }

  Ok(())
}

lazy_static::lazy_static! {
  pub static ref PROGRESS: MultiProgress = MultiProgress::new();
}

fn main() -> Result<()> {
  env_logger::Builder::new()
    .target(env_logger::Target::Stderr)
    .filter_level(log::LevelFilter::Info)
    .parse_env("QWER_LOG")
    .format(|buf, record| {
      let level = match record.level() {
        log::Level::Info => style("==>").bold().cyan(),
        log::Level::Error => style("error:").bold().red(),
        log::Level::Warn => style("warn:").bold().yellow(),
        log::Level::Debug => style("debug:").bold().blue(),
        log::Level::Trace => style("trace:").bold().cyan(),
      };

      writeln!(buf, "{} {}", level, record.args())
    })
    .init();

  let is_asdf = std::env::args().next().context("Failed to get $0")? == "asdf";
  let self_executable = std::env::current_exe().context("Failed to get current executable")?;

  if !is_asdf {
    trace!("Running as qwer ({self_executable:?})");
    ensure_asdf_alias(&self_executable).context("Failed to ensure asdf alias")?;
  } else {
    trace!("Running as asdf ({self_executable:?})");
  }

  let pool = ThreadPool::new(1);

  let result = match Cli::parse().command {
    Commands::Hook { shell } => {
      trace!("Running {} hook", shell.name());
      assert_running_qwer(is_asdf)?;

      let shell_name = shell.name();
      let shell_fns = shell.get();
      let self_executable_str = self_executable.to_string_lossy();
      let hook_cmd = format!("\"{self_executable_str}\" export {shell_name}");
      let hook = shell_fns.hook(&hook_cmd, "qwer_hook");
      print!("{hook}");

      Ok(())
    }
    Commands::Export { shell } => {
      trace!("Exporting {} env", shell.name());
      assert_running_qwer(is_asdf)?;

      let state = cmds::env::update_env()?;
      let set_env = shell.get().apply(&state);

      trace!("Resolved env export:\n{set_env}");
      print!("{set_env}");

      Ok(())
    }
    Commands::Use { name, version } => {
      let plugin_to_use = if let Some(name) = name {
        name
      } else {
        cmds::uuse::select_plugin(&pool)?
      };

      let version_to_install = if let Some(version) = version {
        version
      } else {
        cmds::uuse::select_version(&pool)?
      };

      Ok(())
    }
    Commands::Plugin { command } => match command {
      PluginCommand::Add { name, git_url } => cmds::plugin::add(&pool, name, git_url),
      PluginCommand::List {
        command,
        urls,
        refs,
      } => match command {
        Some(PluginListCommand::All) => cmds::plugin::list_all(&pool),
        None => cmds::plugin::list(&pool, urls, refs),
      },
      PluginCommand::Remove { name } => cmds::plugin::remove(&pool, name),
      PluginCommand::Update {
        command,
        name,
        git_ref,
      } => match (command, name) {
        (Some(PluginUpdateCommand::All), ..) => cmds::plugin::update_all(&pool),
        (None, Some(name)) => cmds::plugin::update(&pool, name, git_ref),
        _ => unreachable!(),
      },
    },
    Commands::Install {
      name,
      version,
      concurrency,
      keep_download,
    } => match (name, version) {
      (None, None) => cmds::install::install_all(&pool, concurrency, keep_download),
      (Some(name), None) => cmds::install::install_one(&pool, name, concurrency, keep_download),
      (Some(name), Some(version)) => {
        cmds::install::install_one_version(&pool, name, version, concurrency, keep_download)
      }
      _ => unreachable!(),
    },
    Commands::Uninstall { name, version } => cmds::install::uninstall(&pool, name, version),
    Commands::Current { name } => cmds::env::current(name),
    Commands::Where { name, version } => cmds::env::wwhere(&pool, name, version),
    Commands::Latest { name, filter } => cmds::list::latest(&pool, name, filter),
    Commands::List {
      command,
      name,
      filter,
    } => match (command, name) {
      (Some(ListCommand::All { name, filter }), None) => cmds::list::all(&pool, name, filter),
      (None, None) => cmds::list::all_installed(),
      (None, Some(name)) => cmds::list::installed(name, filter),
      _ => unreachable!(),
    },
    Commands::Global { name, version } => cmds::version::global(&pool, name, version),
    Commands::Local { name, version } => cmds::version::local(&pool, name, version),
    Commands::Shell { name, version } => cmds::version::shell(&pool, name, version),
    Commands::Help { plugin, version } => cmds::help::help(plugin, version),
    Commands::Command(args) => cmds::ext::ext(args),

    Commands::Reshim { args } => {
      trace!("Skipping legacy command `reshim` ({args:?})");
      Ok(())
    }
    Commands::Which { args } => {
      trace!("Skipping legacy command `which` ({args:?})");
      Ok(())
    }
  };

  match result {
    Err(err) => error!("{}", err),
    _ => {}
  }

  Ok(())
}
