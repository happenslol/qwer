use std::collections::HashMap;

use anyhow::{bail, Result};
use console::style;
use log::{info, trace};
use qwer::versions::{Version, Versions};

use crate::dirs::{get_plugin_scripts, TOOL_VERSIONS};

pub fn install_all() -> Result<()> {
    let to_install = gather_versions()?;
    trace!("Installing versions:\n{to_install:#?}");

    let mut to_install = to_install.iter().collect::<Vec<(&String, &Version)>>();
    to_install.sort_by_key(|(version, _)| version.to_owned());

    for (plugin, version) in to_install {
        let scripts = get_plugin_scripts(&plugin)?;
        if scripts.version_installed(&version) {
            info!("{} {} already installed", &plugin, version.raw());
            continue;
        }

        install(&plugin, &version.raw())?;
    }

    Ok(())
}

pub fn install_one(name: String) -> Result<()> {
    let versions = gather_versions()?;
    if !versions.contains_key(&name) {
        bail!("tool `{name}` is not defined in any version files");
    }

    let to_install = &versions[&name];
    trace!("Installing version: {name} {to_install:?}");

    install(&name, &to_install.raw())
}

fn gather_versions() -> Result<HashMap<String, Version>> {
    let version_files = Versions::find_all(std::env::current_dir()?, TOOL_VERSIONS)?;
    let mut result = HashMap::new();

    for versions in version_files {
        for (plugin, version) in versions.iter() {
            if result.contains_key(plugin) {
                continue;
            }

            let to_install = version.first().unwrap().to_owned();
            if to_install == Version::System {
                continue;
            }

            result.insert(plugin.clone(), to_install);
        }
    }

    Ok(result)
}

pub fn install_one_version(name: String, version: String) -> Result<()> {
    install(&name, &version)
}

fn install(name: &str, version: &str) -> Result<()> {
    let scripts = get_plugin_scripts(&name)?;

    let version = match version {
        "latest" => {
            let latest = scripts.latest()?;

            info!(
                "Resolved {} latest to {}",
                &name,
                style(latest.raw()).bold()
            );

            latest
        }
        "latest-stable" => {
            let latest_stable = scripts.latest_stable()?;

            info!(
                "Resolved {} latest-stable to {}",
                &name,
                style(latest_stable.raw()).bold()
            );

            latest_stable
        }
        _ => scripts.find_version(version)?,
    };

    if let Version::System = version {
        bail!("can't install system version");
    }

    info!("Installing {} {}", &name, version.raw());

    if scripts.has_download() {
        info!("Running download script...");
        let download_output = scripts.download(&version)?;
        trace!("Download output:\n{download_output}");
    }

    info!("Running install script...");
    let install_output = scripts.install(&version)?;
    trace!("Install output:\n{install_output}");

    info!("Installed {} {}", &name, version.raw());

    Ok(())
}

pub fn uninstall(name: String, version: String) -> Result<()> {
    let scripts = get_plugin_scripts(&name)?;
    let version = Version::parse(&version);
    if !scripts.version_installed(&version) {
        bail!("version `{}` is not installed", version.version_str());
    }

    info!("Uninstalling {} {}", &name, version.raw());

    if scripts.has_uninstall() {
        info!("Running uninstall script...");
        let uninstall_output = scripts.uninstall(&version)?;
        trace!("Uninstall ouput:\n{uninstall_output}");
    } else {
        info!("Running version directory...");
        scripts.rm_version(&version)?;
    }

    info!("Uninstalled {} {}", &name, version.raw());

    Ok(())
}
