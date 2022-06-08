use anyhow::{bail, Result};
use console::style;
use log::{info, trace};
use qwer::versions::{Version, Versions};

use crate::dirs::{get_plugin_scripts, TOOL_VERSIONS};

pub fn install_all_local() -> Result<()> {
    let _versions = Versions::find(std::env::current_dir()?, TOOL_VERSIONS)?;
    todo!()
}

pub fn install_one_local(_name: String) -> Result<()> {
    let _versions = Versions::find(std::env::current_dir()?, TOOL_VERSIONS)?;
    todo!()
}

pub fn install_one_version(name: String, version: String) -> Result<()> {
    let scripts = get_plugin_scripts(&name)?;

    let version = match version.as_str() {
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
