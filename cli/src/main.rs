use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use qwer::Shell;

mod install;
mod plugin;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
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
    },

    Uninstall {
        name: String,
        version: String,
    },

    Latest {
        name: String,
        version: Option<String>,
    },

    List {
        #[clap(subcommand)]
        command: Option<ListCommand>,

        name: Option<String>,
        version: Option<String>,
    },

    Global {
        name: String,
        version: Vec<String>,
    },

    Local {
        name: String,
        version: Vec<String>,
    },

    Shell {
        name: String,
        version: Vec<String>,
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
        Commands::Hook { shell } => command_hook(shell),
        Commands::Export { shell } => command_export(shell),
        Commands::Plugin { command } => command_plugin(command),
        Commands::Install { name, version } => command_install(name, version),
        Commands::Uninstall { name, version } => command_uninstall(name, version),
        Commands::Latest { name, version } => todo!(),
        Commands::List {
            command,
            name,
            version,
        } => todo!(),
        Commands::Global { name, version } => todo!(),
        Commands::Local { name, version } => todo!(),
        Commands::Shell { name, version } => todo!(),
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
        PluginCommand::Update {
            command,
            name,
            git_ref,
        } => match (command, name) {
            (Some(PluginUpdateCommand::All), ..) => plugin::update_all(),
            (None, Some(name)) => plugin::update(name, git_ref),
            _ => unreachable!(),
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

fn command_uninstall(name: String, version: String) -> Result<()> {
    install::uninstall(name, version)
}
