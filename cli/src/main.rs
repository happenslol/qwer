use std::collections::HashMap;

use clap::{Parser, Args, Subcommand};
use qwer::Shell;

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

impl ShellOptions {
    fn hook(&self, cmd: &str, hook_fn: &str) -> String {
        match self {
            ShellOptions::Bash => qwer::Bash::hook(cmd, hook_fn),
            ShellOptions::Zsh => qwer::Zsh::hook(cmd, hook_fn),
        }
    }

    fn export(&self, env: &qwer::Env) -> String {
        match self {
            ShellOptions::Bash => qwer::Bash::export(env),
            ShellOptions::Zsh => qwer::Zsh::export(env),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            ShellOptions::Bash => "bash",
            ShellOptions::Zsh => "zsh",
        }
    }
}

fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Hook(hook) => command_hook(&hook.shell),
        Commands::Export(export) => command_export(&export.shell),
    }
}

fn command_hook(shell: &ShellOptions) {
    let self_path = std::env::args().next().expect("Failed to get executable path");
    let shell_name = shell.name();

    let hook_cmd = format!("\"{self_path}\" export {shell_name}");
    let hook = shell.hook(&hook_cmd, "qwer_hook");
    print!("{hook}");
}

fn command_export(shell: &ShellOptions) {
    let env = qwer::Env {
        path: vec![],
        vars: HashMap::from([
            ("foo", "bar"),
        ])
    };

    let hook = shell.export(&env);
    print!("{hook}");
}
