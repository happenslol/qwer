use std::{fs, path::PathBuf};

use anyhow::{anyhow, Result};
use qwer::scripts::PluginScripts;

pub const REGISTRIES_DIR: &str = "registries";
pub const PLUGINS_DIR: &str = "plugins";
pub const INSTALLS_DIR: &str = "installs";
pub const DOWNLOADS_DIR: &str = "downloads";

const TOOL_VERSIONS: &str = ".tool-versions";
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

pub fn get_global_tool_versions() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("failed to get home dir"))?;
    Ok(home_dir.join(TOOL_VERSIONS))
}

pub fn get_plugin_scripts(name: &str) -> Result<PluginScripts> {
    Ok(PluginScripts::new(
        &name,
        &get_dir(PLUGINS_DIR)?,
        &get_dir(INSTALLS_DIR)?,
        &get_dir(DOWNLOADS_DIR)?,
    )?)
}
