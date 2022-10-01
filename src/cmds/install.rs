use std::collections::HashMap;

use anyhow::{bail, Result};
use console::style;
use log::{info, trace};

use crate::{
  dirs::{get_plugin_scripts, TOOL_VERSIONS},
  versions::{Version, Versions},
};

pub fn install_all(concurrency: Option<usize>, keep_download: bool) -> Result<()> {
  let to_install = gather_versions()?;
  trace!("Installing versions:\n{to_install:#?}");

  let mut to_install = to_install.iter().collect::<Vec<(&String, &Version)>>();
  to_install.sort_by_key(|(version, _)| version.to_owned());

  for (plugin, version) in to_install {
    let scripts = get_plugin_scripts(plugin)?;
    if scripts.version_installed(version) {
      info!("{} {} already installed", &plugin, version.raw());
      continue;
    }

    install(plugin, &version.raw(), concurrency, keep_download)?;
  }

  Ok(())
}

pub fn install_one(name: String, concurrency: Option<usize>, keep_download: bool) -> Result<()> {
  let versions = gather_versions()?;
  if !versions.contains_key(&name) {
    bail!("Tool `{name}` is not defined in any version files");
  }

  let to_install = &versions[&name];
  trace!("Installing version: {name} {to_install:?}");

  install(&name, &to_install.raw(), concurrency, keep_download)
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

pub fn install_one_version(
  name: String,
  version: String,
  concurrency: Option<usize>,
  keep_download: bool,
) -> Result<()> {
  install(&name, &version, concurrency, keep_download)
}

fn install(
  name: &str,
  version: &str,
  concurrency: Option<usize>,
  keep_download: bool,
) -> Result<()> {
  let scripts = get_plugin_scripts(name)?;
  let resolved = scripts.resolve(version)?;
  if resolved.is_none() {
    bail!(
      "Failed to resolve version {} for plugin {}",
      style(version).bold(),
      style(name).bold()
    );
  }

  let resolved = resolved.unwrap();
  if version != resolved.raw() {
    info!("Resolved {} to {}", version, resolved.raw());
  }

  if let Version::System = resolved {
    bail!("Can't install system version");
  }

  info!("Installing {} {}", &name, resolved.raw());

  if scripts.has_download() {
    scripts.download(&resolved)?;
  }

  scripts.install(&resolved, concurrency)?;

  info!(
    "Installed {} {}",
    style(name).bold(),
    style(resolved.raw()).bold()
  );

  if !keep_download {
    scripts.rm_version_download(&resolved)?;
  }

  Ok(())
}

pub fn uninstall(name: String, version: String) -> Result<()> {
  let scripts = get_plugin_scripts(&name)?;
  let version = Version::parse(&version);
  if !scripts.version_installed(&version) {
    bail!("version {} is not installed", version.version_str());
  }

  info!("Uninstalling {} {}", &name, version.raw());

  if scripts.has_uninstall() {
    scripts.uninstall(&version)?;
  } else {
    scripts.rm_version(&version)?;
  }

  // Just in case this wasn't cleaned earlier
  scripts.rm_version_download(&version)?;

  info!("Uninstalled {} {}", &name, version.raw());

  Ok(())
}
