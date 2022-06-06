use anyhow::Result;
use qwer::{versions::Version, scripts::PluginScripts};

use crate::{get_dir, DOWNLOADS_DIR, INSTALLS_DIR, PLUGINS_DIR};

pub fn install_all_local() -> Result<()> {
    Ok(())
}

pub fn install_one_local(_name: String) -> Result<()> {
    Ok(())
}

pub fn install_one_version(name: String, version: String) -> Result<()> {
    let scripts = PluginScripts::new(
        &name,
        &get_dir(PLUGINS_DIR)?,
        &get_dir(INSTALLS_DIR)?,
        &get_dir(DOWNLOADS_DIR)?,
    )?;

    let version = Version::parse(&version);
    let _download_output = scripts.download(&version)?;
    let install_output = scripts.install(&version)?;

    println!("{install_output}");
    Ok(())
}

pub fn uninstall(name: String, version: String) -> Result<()> {
    let scripts = PluginScripts::new(
        &name,
        &get_dir(PLUGINS_DIR)?,
        &get_dir(INSTALLS_DIR)?,
        &get_dir(DOWNLOADS_DIR)?,
    )?;

    let version = Version::parse(&version);
    let uninstall_output = scripts.uninstall(&version)?;

    println!("{uninstall_output}");
    Ok(())
}
