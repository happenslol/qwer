use std::{
  collections::HashMap,
  fs,
  path::Path,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Result};
use console::style;
use log::trace;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  dirs::{get_data_dir, get_dir, get_plugin_scripts, PLUGINS_DIR, REGISTRIES_DIR},
  git,
  process::auto_bar,
};

const DEFAULT_PLUGIN_REGISTRY_URL: &str = "https://github.com/asdf-vm/asdf-plugins.git";
const DEFAULT_PLUGIN_REGISTRY: &str = "default";
const REGISTRY_CONFIG: &str = "registries.toml";

#[derive(Error, Debug)]
pub enum RegistryError {
  #[error("Plugin `{0}` was not found in the plugin repo")]
  NotFound(String),

  #[error("IO error while looking for plugin")]
  Io(#[from] std::io::Error),

  #[error("Plugin shortcut `{0}` should be in format `repository = <git-url>`")]
  InvalidFile(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Registry {
  pub last_sync: u64,
}

fn save_registries(regs: &HashMap<String, Registry>) -> Result<()> {
  let registry_config_path = get_data_dir()?.join(REGISTRY_CONFIG);
  let serialized = toml::to_string(regs)?;
  fs::write(registry_config_path, serialized)?;

  Ok(())
}

fn load_registries() -> Result<HashMap<String, Registry>> {
  let registry_config_path = get_data_dir()?.join(REGISTRY_CONFIG);
  if !registry_config_path.is_file() {
    return Ok(HashMap::new());
  }

  let contents = fs::read_to_string(registry_config_path)?;
  Ok(toml::from_str(&contents)?)
}

fn update_registry(url: &str, name: &str, force: bool) -> Result<()> {
  let registry_dir = get_dir(REGISTRIES_DIR)?.join(name);
  let message = format!("Cloning plugin registry {}", style(name).bold());

  if !registry_dir.is_dir() {
    let registries_dir = get_dir(REGISTRIES_DIR)?;
    let bar = auto_bar();
    git::GitRepo::clone((&bar, &message), &registries_dir, url, name, None)?;
  } else {
    let mut registries = load_registries()?;
    let last_sync = registries.get(name).map(|reg| reg.last_sync).unwrap_or(0);
    let elapsed = (UNIX_EPOCH + Duration::from_secs(last_sync)).elapsed()?;

    trace!(
      "Plugin repo `{}` was updated {}s ago",
      name,
      elapsed.as_secs()
    );

    if elapsed < Duration::from_secs(60 * 60) && !force {
      return Ok(());
    }

    let repo = git::GitRepo::new(&registry_dir)?;
    repo.update_to_remote_head(None, None)?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    registries.insert(name.to_owned(), Registry { last_sync: now });
    save_registries(&registries)?;
  }

  Ok(())
}

pub fn add(name: String, git_url: Option<String>) -> Result<()> {
  let plugin_dir = get_dir(PLUGINS_DIR)?;
  let add_plugin_dir = plugin_dir.join(&name);
  if add_plugin_dir.is_dir() {
    bail!("Plugin with name `{name}` is already installed");
  }

  let git_url = match git_url {
    Some(git_url) => git_url,
    None => {
      let registry_dir = get_dir(REGISTRIES_DIR)?.join(DEFAULT_PLUGIN_REGISTRY);
      parse_short_repo_url(registry_dir, &name)?
    }
  };

  let bar = auto_bar();
  git::GitRepo::clone(
    (
      &bar,
      &format!("Installing plugin {}", style(&name).blue().bold()),
    ),
    &plugin_dir,
    &git_url,
    &name,
    None,
  )?;

  let scripts = get_plugin_scripts(&name)?;
  scripts.post_plugin_add(&git_url)?;

  Ok(())
}

fn normalize_repo_url(url: &str) -> String {
  url
    .trim_start_matches("https://")
    .trim_start_matches("git@")
    .replace(':', "/")
}

pub struct PluginListEntry {
  pub name: String,
  pub url: String,
  pub rref: String,
  pub installed: bool,
}

pub fn list(force_refresh: bool) -> Result<Vec<PluginListEntry>> {
  update_registry(
    DEFAULT_PLUGIN_REGISTRY_URL,
    DEFAULT_PLUGIN_REGISTRY,
    force_refresh,
  )?;

  let plugin_dir = get_dir(PLUGINS_DIR)?;
  Ok(
    fs::read_dir(&plugin_dir)?
      .map(|dir| {
        let dir = dir?;

        let name = String::from(dir.file_name().to_string_lossy());
        let repo = git::GitRepo::new(dir.path())?;

        let url = repo.get_remote_url()?;

        let branch = repo.get_head_branch()?;
        let gitref = repo.get_head_ref()?;
        let rref = format!("{branch} {gitref}");

        Ok(PluginListEntry {
          name,
          url,
          rref,
          installed: true,
        })
      })
      .collect::<Result<Vec<_>>>()?,
  )
}

pub fn list_all(force_refresh: bool) -> Result<Vec<PluginListEntry>> {
  update_registry(
    DEFAULT_PLUGIN_REGISTRY_URL,
    DEFAULT_PLUGIN_REGISTRY,
    force_refresh,
  )?;

  let registry_dir = get_dir(REGISTRIES_DIR)?.join(DEFAULT_PLUGIN_REGISTRY);
  let plugins_dir = get_dir(PLUGINS_DIR)?;

  Ok(
    fs::read_dir(registry_dir.join("plugins"))?
      .map(|plugin| {
        let plugin = plugin?;
        let name = String::from(plugin.file_name().to_string_lossy());
        let url = parse_short_repo_url(&registry_dir, &name)?;

        let installed_plugin_dir = plugins_dir.join(&name);
        let (installed, rref) = if installed_plugin_dir.is_dir() {
          let repo = git::GitRepo::new(&installed_plugin_dir)?;
          let remote_url = repo.get_remote_url()?;

          let installed_url = normalize_repo_url(&remote_url);
          let registry_url = normalize_repo_url(&remote_url);

          let branch = repo.get_head_branch()?;
          let gitref = repo.get_head_ref()?;
          let rref = format!("{branch} {gitref}");

          (installed_url == registry_url, rref)
        } else {
          (false, String::new())
        };

        Ok(PluginListEntry {
          name,
          url,
          rref,
          installed,
        })
      })
      .collect::<Result<Vec<_>>>()?,
  )
}

/// Retrieve the repository url from a directory containing plugin references.
/// See [the asdf plugin repository](https://github.com/asdf-vm/asdf-plugins/tree/master/plugins)
/// for the expected file format and contents.
pub fn parse_short_repo_url<P: AsRef<Path>>(
  registry: P,
  plugin: &str,
) -> Result<String, RegistryError> {
  let reg_path = registry.as_ref();
  trace!("Parsing short plugin `{plugin}` from registry at `{reg_path:?}`");

  let plugin_file = reg_path.join("plugins").join(plugin);
  if !plugin_file.is_file() {
    trace!("Plugin file for `{plugin}` not found at `{plugin_file:?}`");
    return Err(RegistryError::NotFound(plugin.to_owned()));
  }

  let contents = fs::read_to_string(plugin_file)?;
  let parts = contents.split('=').collect::<Vec<&str>>();
  if parts.len() != 2 || parts[0].trim() != "repository" {
    trace!("Failed to parse contents `{contents}` into plugin url");
    return Err(RegistryError::InvalidFile(contents));
  }

  Ok(parts[1].trim().to_owned())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_short() {
    let workdir = tempfile::tempdir().expect("failed to create temp dir");
    let plugins = workdir.path().join("plugins");
    fs::create_dir_all(&plugins).expect("failed to create plugins dir");

    fs::write(plugins.join("foo"), "repository = bar").expect("failed to write plugin file");

    let result = parse_short_repo_url(&workdir, "foo").expect("failed to parse");
    assert_eq!(result, "bar");
  }

  #[test]
  fn parse_not_found() {
    let workdir = tempfile::tempdir().expect("failed to create temp dir");
    let plugins = workdir.path().join("plugins");
    fs::create_dir_all(&plugins).expect("failed to create plugins dir");

    fs::write(plugins.join("foo"), "repository = bar").expect("failed to write plugin file");

    let result = parse_short_repo_url(&workdir, "bar");
    assert!(matches!(result, Err(RegistryError::NotFound(_))));
  }

  #[test]
  fn parse_invalid_format() {
    let workdir = tempfile::tempdir().expect("failed to create temp dir");
    let plugins = workdir.path().join("plugins");
    fs::create_dir_all(&plugins).expect("failed to create plugins dir");

    fs::write(plugins.join("foo"), "invalid format").expect("failed to write plugin file");

    let result = parse_short_repo_url(&workdir, "foo");
    dbg!(&result);
    assert!(matches!(result, Err(RegistryError::InvalidFile(_))));
  }
}
