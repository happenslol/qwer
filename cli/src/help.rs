use std::io::Write;

use crate::{dirs::get_plugin_scripts, Cli};
use anyhow::{bail, Result};
use clap::IntoApp;
use qwer::versions::Version;

pub fn help(plugin: Option<String>, version: Option<String>) -> Result<()> {
    if plugin.is_none() {
        Cli::command().print_help().unwrap();

        // See https://github.com/clap-rs/clap/blob/a96e7cfc7fc155e86f4e08767b934bfcb666b665/src/util/mod.rs#L23
        let _ = std::io::stdout().lock().flush();
        let _ = std::io::stderr().lock().flush();
        std::process::exit(2);
    }

    let plugin = plugin.unwrap();
    let scripts = get_plugin_scripts(&plugin)?;
    let version = version.map(|raw| Version::parse(&raw));

    let overview = scripts.help_overview(version.as_ref())?;
    if overview.is_none() {
        bail!("No help for plugin `{plugin}`");
    }

    let overview = overview.unwrap();
    println!("{overview}");

    if let Some(content) = scripts.help_deps(version.as_ref())? {
        println!("{content}");
    }

    if let Some(content) = scripts.help_config(version.as_ref())? {
        println!("{content}");
    }

    if let Some(content) = scripts.help_links(version.as_ref())? {
        println!("{content}");
    }

    Ok(())
}
