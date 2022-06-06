use std::{fs, path::PathBuf};

use anyhow::{bail, Result};

use crate::{get_data_dir, plugin, scripts::run_script};

const INSTALLS_DIR: &str = "installs";

fn get_installs_dir() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    let plugin_dir = data_dir.join(INSTALLS_DIR);
    fs::create_dir_all(&plugin_dir)?;
    Ok(plugin_dir)
}

pub fn install_all_local() -> Result<()> {
    Ok(())
}

pub fn install_one_local(_name: String) -> Result<()> {
    Ok(())
}

pub fn install_one_version(name: String, _version: String) -> Result<()> {
    let plugin_dir = plugin::get_plugins_dir()?.join(&name);
    if !plugin_dir.is_dir() {
        bail!("plugin `{name}` not found");
    }

    let list_all = plugin_dir.join("bin/list-all");
    let output = run_script(&list_all)?;

    let versions = output
        .trim()
        .split(' ')
        .map(|version| version.trim())
        .collect::<Vec<_>>();

    println!("{versions:#?}");

    let _installs_dir = get_installs_dir()?;

    Ok(())
}
