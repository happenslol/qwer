use std::{
    io::Write,
    path::{Path, PathBuf},
};

use crate::dirs::{get_dir, BIN_DIR};
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use console::style;
use log::trace;
use qwer::shell::Shell;

mod dirs;
mod env;
mod help;
mod install;
mod list;
mod plugin;
mod version;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
#[clap(disable_help_subcommand(true))]
enum Commands {
    Hook {
        #[clap(subcommand)]
        shell: ShellOptions,
    },

    Export {
        #[clap(subcommand)]
        shell: ShellOptions,
    },

    Plugin {
        #[clap(subcommand)]
        command: PluginCommand,
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

    // Legacy commands
    Reshim {
        args: Vec<String>,
    },
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
            ShellOptions::Bash => &qwer::shell::Bash,
            ShellOptions::Zsh => &qwer::shell::Zsh,
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

fn assert_running_qwer(is_running_asdf_alias: bool) -> Result<()> {
    if !is_running_asdf_alias {
        bail!("This command can not be run from an asdf symlink");
    }

    Ok(())
}

fn main() -> Result<()> {
    env_logger::Builder::new()
        .target(env_logger::Target::Stderr)
        .filter_level(log::LevelFilter::Info)
        .parse_env("QWER_LOG")
        .format(|buf, record| {
            if let log::Level::Info = record.level() {
                writeln!(buf, "{}", record.args())
            } else {
                let level = match record.level() {
                    log::Level::Error => style(" error ").black().on_red(),
                    log::Level::Warn => style(" warn ").black().on_yellow(),
                    log::Level::Debug => style(" debug ").black().on_blue(),
                    log::Level::Trace => style(" trace ").black().on_cyan(),
                    _ => unreachable!(),
                };

                writeln!(buf, "{} {}", level, record.args())
            }
        })
        .init();

    let mut self_path = std::env::args()
        .next()
        .context("Failed to get executable path")?;

    let is_asdf = self_path == "asdf";
    if !is_asdf {
        trace!("Running as qwer");
        let self_path_canon = PathBuf::from(self_path)
            .canonicalize()
            .context("Failed to canonicalize self path")?;

        ensure_asdf_alias(self_path_canon.as_path()).context("Failed to ensure asdf alias")?;
        self_path = self_path_canon.to_string_lossy().to_string();
    } else {
        trace!("Running as asdf");
    }

    match Cli::parse().command {
        Commands::Hook { shell } => {
            trace!("Running {} hook", shell.name());
            assert_running_qwer(is_asdf)?;

            let shell_name = shell.name();
            let shell_fns = shell.get();
            let hook_cmd = format!("\"{self_path}\" export {shell_name}");
            let hook = shell_fns.hook(&hook_cmd, "qwer_hook");
            print!("{hook}");

            Ok(())
        }
        Commands::Export { shell } => {
            trace!("Exporting {} env", shell.name());
            assert_running_qwer(is_asdf)?;

            let state = env::update_env()?;
            let set_env = shell.get().apply(&state);

            trace!("Resolved env export:\n{set_env}");
            print!("{set_env}");

            Ok(())
        }
        Commands::Plugin { command } => match command {
            PluginCommand::Add { name, git_url } => plugin::add(name, git_url),
            PluginCommand::List {
                command,
                urls,
                refs,
            } => match command {
                Some(PluginListCommand::All) => plugin::list_all(),
                None => plugin::list(urls, refs),
            },
            PluginCommand::Remove { name } => plugin::remove(name),
            PluginCommand::Update {
                command,
                name,
                git_ref,
            } => match (command, name) {
                (Some(PluginUpdateCommand::All), ..) => plugin::update_all(),
                (None, Some(name)) => plugin::update(name, git_ref),
                _ => unreachable!(),
            },
        },
        Commands::Install {
            name,
            version,
            concurrency,
            keep_download,
        } => match (name, version) {
            (None, None) => install::install_all(concurrency, keep_download),
            (Some(name), None) => install::install_one(name, concurrency, keep_download),
            (Some(name), Some(version)) => {
                install::install_one_version(name, version, concurrency, keep_download)
            }
            _ => unreachable!(),
        },
        Commands::Uninstall { name, version } => install::uninstall(name, version),
        Commands::Current { .. } => todo!(),
        Commands::Where { .. } => todo!(),
        Commands::Latest { name, filter } => list::latest(name, filter),
        Commands::List {
            command,
            name,
            filter,
        } => match (command, name) {
            (Some(ListCommand::All { name, filter }), None) => list::all(name, filter),
            (None, None) => list::all_installed(),
            (None, Some(name)) => list::installed(name, filter),
            _ => unreachable!(),
        },
        Commands::Global { name, version } => version::global(name, version),
        Commands::Local { name, version } => version::local(name, version),
        Commands::Shell { name, version } => version::shell(name, version),
        Commands::Help { plugin, version } => help::help(plugin, version),
        Commands::Reshim { args } => {
            trace!("Skipping legacy command `reshim` ({args:?})");
            Ok(())
        }
    }
}
