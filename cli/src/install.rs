use std::fs;

use anyhow::{bail, Result};

use crate::{get_dir, scripts::run_script, DOWNLOADS_DIR, INSTALLS_DIR, PLUGINS_DIR};

pub fn install_all_local() -> Result<()> {
    Ok(())
}

pub fn install_one_local(_name: String) -> Result<()> {
    Ok(())
}

pub fn install_one_version(name: String, version: String) -> Result<()> {
    let plugin_dir = get_dir(PLUGINS_DIR)?.join(&name);
    if !plugin_dir.is_dir() {
        bail!("plugin `{name}` not found");
    }

    let install_dir = get_dir(INSTALLS_DIR)?.join(&name).join(&version);
    if install_dir.is_dir() {
        bail!("version `{version}` is already installed");
    }

    fs::create_dir_all(&install_dir)?;

    let download_script = plugin_dir.join("bin/download");
    let download_dir = get_dir(DOWNLOADS_DIR)?.join(&name).join(&version);
    if download_script.is_file() {
        fs::create_dir_all(&download_dir)?;

        println!("running download script");
        let output = run_script(
            &download_script.to_string_lossy(),
            &[
                ("ASDF_INSTALL_TYPE", "version"),
                ("ASDF_INSTALL_VERSION", &version),
                ("ASDF_INSTALL_PATH", &install_dir.to_string_lossy()),
                ("ASDF_DOWNLOAD_PATH", &download_dir.to_string_lossy()),
            ],
        )?;

        println!("{output}");
    }

    let install_script = plugin_dir.join("bin/install");
    if !install_script.is_file() {
        bail!("install script for `{name}` not found");
    }

    println!("running install script");
    let output = run_script(
        &install_script.to_string_lossy(),
        &[
            ("ASDF_INSTALL_TYPE", "version"),
            ("ASDF_INSTALL_VERSION", &version),
            ("ASDF_INSTALL_PATH", &install_dir.to_string_lossy()),
            ("ASDF_DOWNLOAD_PATH", &download_dir.to_string_lossy()),
            ("ASDF_CONCURRENCY", "1"),
        ],
    )?;

    println!("{output}");

    Ok(())
}
