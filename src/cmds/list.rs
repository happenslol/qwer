use std::fs::{self, DirEntry};

use anyhow::{bail, Result};

use crate::dirs::{get_dir, get_plugin_scripts, INSTALLS_DIR};

pub fn all_installed() -> Result<()> {
  let install_dir = get_dir(INSTALLS_DIR)?;

  let entries = fs::read_dir(&install_dir)?
    .collect::<Result<Vec<DirEntry>, std::io::Error>>()?
    .iter()
    .map(|entry| entry.file_name().to_string_lossy().to_string())
    .collect::<Vec<_>>();

  if entries.is_empty() {
    println!("No tools installed");
    return Ok(());
  }

  for plugin in entries {
    let installed = get_installed_versions(&plugin, None)?;
    if installed.is_empty() {
      continue;
    }

    println!("{plugin}");
    for version in installed {
      println!("  {version}");
    }
    println!();
  }

  Ok(())
}

pub fn installed(name: String, filter: Option<String>) -> Result<()> {
  let installed = get_installed_versions(&name, filter)?;
  for version in installed {
    println!("{version}");
  }

  Ok(())
}

fn get_installed_versions(name: &str, filter: Option<String>) -> Result<Vec<String>> {
  let install_dir = get_dir(INSTALLS_DIR)?.join(&name);
  if !install_dir.is_dir() {
    bail!("No versions installed for `{name}`");
  }

  let entries = fs::read_dir(&install_dir)?
    .collect::<Result<Vec<DirEntry>, std::io::Error>>()?
    .iter()
    .map(|entry| entry.file_name().to_string_lossy().to_string())
    .collect::<Vec<_>>();

  let filtered = if let Some(filter) = filter {
    entries
      .into_iter()
      .filter(|version| version.starts_with(&filter))
      .collect()
  } else {
    entries
  };

  Ok(filtered)
}

fn get_available_versions(name: &str, filter: Option<String>) -> Result<Vec<String>> {
  let scripts = get_plugin_scripts(name)?;
  let versions = scripts.list_all()?;

  let filtered = if let Some(filter) = filter {
    versions
      .into_iter()
      .filter(|version| version.starts_with(&filter))
      .collect::<Vec<_>>()
  } else {
    versions
  };

  Ok(filtered)
}

pub fn all(name: String, filter: Option<String>) -> Result<()> {
  let versions = get_available_versions(&name, filter)?;
  if versions.is_empty() {
    bail!("No versions found");
  }

  for version in versions {
    println!("{version}");
  }

  Ok(())
}

pub fn latest(name: String, filter: Option<String>) -> Result<()> {
  let versions = get_available_versions(&name, filter)?;
  if versions.is_empty() {
    bail!("No versions found");
  }

  println!("{}", versions.last().unwrap());

  Ok(())
}
