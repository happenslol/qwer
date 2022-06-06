use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
};

use anyhow::{anyhow, bail, Result};
use clap::{Args, Parser, Subcommand};
use qwer::Shell;

mod install;
mod plugin;
mod scripts;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Hook(Hook),
    Export(Export),

    Plugin(Plugin),

    Install {
        name: Option<String>,
        version: Option<String>,
    },
}

#[derive(Debug, Args)]
struct Hook {
    #[clap(subcommand)]
    shell: ShellOptions,
}

#[derive(Debug, Args)]
struct Export {
    #[clap(subcommand)]
    shell: ShellOptions,
}

#[derive(Debug, Subcommand)]
enum ShellOptions {
    Bash,
    Zsh,
}

#[derive(Debug, Args)]
struct Plugin {
    #[clap(subcommand)]
    command: PluginCommand,
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
        name: Option<String>,
        git_ref: Option<String>,

        #[clap(short, long)]
        all: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PluginListCommand {
    All,
}

impl ShellOptions {
    fn hook(&self, cmd: &str, hook_fn: &str) -> String {
        match self {
            ShellOptions::Bash => qwer::shell::Bash::hook(cmd, hook_fn),
            ShellOptions::Zsh => qwer::shell::Zsh::hook(cmd, hook_fn),
        }
    }

    fn export(&self, env: &qwer::Env) -> String {
        match self {
            ShellOptions::Bash => qwer::shell::Bash::export(env),
            ShellOptions::Zsh => qwer::shell::Zsh::export(env),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            ShellOptions::Bash => "bash",
            ShellOptions::Zsh => "zsh",
        }
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Hook(hook) => command_hook(hook.shell),
        Commands::Export(export) => command_export(export.shell),
        Commands::Plugin(plugin) => command_plugin(plugin.command),
        Commands::Install { name, version } => command_install(name, version),
    }
}

pub const REGISTRIES_DIR: &str = "registries";
pub const PLUGINS_DIR: &str = "plugins";
pub const INSTALLS_DIR: &str = "installs";
pub const DOWNLOADS_DIR: &str = "downloads";

const DATA_DIR: &str = "qwer";

pub fn get_data_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().ok_or_else(|| anyhow!("failed to get data dir"))?;
    let qwer_data_dir = data_dir.join(DATA_DIR);
    fs::create_dir_all(&qwer_data_dir)?;
    Ok(qwer_data_dir)
}

pub fn get_dir(dir: &str) -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    let subdir = data_dir.join(dir);
    fs::create_dir_all(&subdir)?;
    Ok(subdir)
}

fn command_hook(shell: ShellOptions) -> Result<()> {
    let self_path = std::env::args()
        .next()
        .expect("Failed to get executable path");

    let shell_name = shell.name();
    let hook_cmd = format!("\"{self_path}\" export {shell_name}");
    let hook = shell.hook(&hook_cmd, "qwer_hook");
    print!("{hook}");

    Ok(())
}

fn command_export(shell: ShellOptions) -> Result<()> {
    let env = qwer::Env {
        path: vec![],
        vars: HashMap::from([("foo", "bar")]),
    };

    let export = shell.export(&env);
    print!("{export}");

    Ok(())
}

fn command_plugin(plugin: PluginCommand) -> Result<()> {
    match plugin {
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
        PluginCommand::Update { name, git_ref, all } => match (name, all) {
            (Some(name), false) => plugin::update(name, git_ref),
            (None, true) => plugin::update_all(),
            _ => bail!("plugin name or --all must be given"),
        },
    }
}

fn command_install(name: Option<String>, version: Option<String>) -> Result<()> {
    match (name, version) {
        (None, None) => install::install_all_local(),
        (Some(name), None) => install::install_one_local(name),
        (Some(name), Some(version)) => install::install_one_version(name, version),
        _ => unreachable!(),
    }
}
