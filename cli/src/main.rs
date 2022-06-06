use std::collections::HashMap;

use anyhow::Result;
use clap::{Parser, Subcommand};
use qwer::Shell;

mod dirs;
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
    All {
        name: String,
        filter: Option<String>,
    },
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
        Commands::Hook { shell } => {
            let self_path = std::env::args()
                .next()
                .expect("Failed to get executable path");

            let shell_name = shell.name();
            let hook_cmd = format!("\"{self_path}\" export {shell_name}");
            let hook = shell.hook(&hook_cmd, "qwer_hook");
            print!("{hook}");

            Ok(())
        }
        Commands::Export { shell } => {
            let env = qwer::Env {
                path: vec![],
                vars: HashMap::from([("foo", "bar")]),
            };

            let export = shell.export(&env);
            print!("{export}");

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
        Commands::Install { name, version } => match (name, version) {
            (None, None) => install::install_all_local(),
            (Some(name), None) => install::install_one_local(name),
            (Some(name), Some(version)) => install::install_one_version(name, version),
            _ => unreachable!(),
        },
        Commands::Uninstall { name, version } => install::uninstall(name, version),
        Commands::Latest { name, filter } => list::latest(name, filter),
        Commands::List {
            command,
            name,
            filter,
        } => match (command, name) {
            (Some(ListCommand::All { name, filter }), None) => list::all(name, filter),
            (None, Some(name)) => list::installed(name, filter),
            _ => unreachable!(),
        },
        Commands::Global { name, version } => version::global(name, version),
        Commands::Local { name, version } => version::local(name, version),
        Commands::Shell { name, version } => version::shell(name, version),
    }
}
