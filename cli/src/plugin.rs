use std::{fs, time::Duration};

use anyhow::{bail, Result};
use log::info;
use qwer::plugins::parse_short_repo_url;
use tabled::{object::Segment, Alignment, Modify, Table, Tabled};

use crate::dirs::{get_dir, PLUGINS_DIR, REGISTRIES_DIR};

const DEFAULT_PLUGIN_REGISTRY_URL: &str = "https://github.com/asdf-vm/asdf-plugins.git";
const DEFAULT_PLUGIN_REGISTRY: &str = "default";

fn update_registry(url: &str, name: &str, _force: bool) -> Result<()> {
    let registry_dir = get_dir(REGISTRIES_DIR)?.join(name);

    if !registry_dir.is_dir() {
        info!("Initializing registry `{name}`...");
        let registries_dir = get_dir(REGISTRIES_DIR)?;
        git::GitRepo::clone(&registries_dir, url, name, None)?;
    } else {
        let modified = fs::metadata(&registry_dir)?.modified()?;
        if modified.elapsed()? < Duration::from_secs(60 * 1000) {
            return Ok(());
        }

        println!("updating plugin repo...");
        let repo = git::GitRepo::new(&registry_dir)?;
        repo.update_to_remote_head()?;
    }

    Ok(())
}

pub fn add(name: String, git_url: Option<String>) -> Result<()> {
    let plugin_dir = get_dir(PLUGINS_DIR)?;
    let add_plugin_dir = plugin_dir.join(&name);
    if add_plugin_dir.is_dir() {
        bail!("plugin with name `{name}` is already installed");
    }

    let git_url = match git_url {
        Some(git_url) => git_url,
        None => {
            let registry_dir = get_dir(REGISTRIES_DIR)?.join(DEFAULT_PLUGIN_REGISTRY);
            parse_short_repo_url(registry_dir, &name)?
        }
    };

    git::GitRepo::clone(&plugin_dir, &git_url, &name, None)?;

    Ok(())
}

fn normalize_repo_url(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("git@")
        .replace(':', "/")
}

fn display_option(opt: &Option<String>) -> String {
    match opt {
        Some(s) => s.clone(),
        None => String::new(),
    }
}

#[derive(Tabled)]
struct ListItem {
    name: String,

    #[tabled(display_with = "display_option")]
    url: Option<String>,

    #[tabled(rename = "ref", display_with = "display_option")]
    rref: Option<String>,
}

pub fn list(urls: bool, refs: bool) -> Result<()> {
    update_registry(DEFAULT_PLUGIN_REGISTRY_URL, DEFAULT_PLUGIN_REGISTRY, false)?;

    let plugin_dir = get_dir(PLUGINS_DIR)?;
    let plugins = fs::read_dir(&plugin_dir)?
        .map(|dir| {
            let dir = dir?;

            let name = String::from(dir.file_name().to_string_lossy());
            let (url, rref) = if urls || refs {
                let repo = git::GitRepo::new(dir.path())?;

                let url = if urls {
                    Some(repo.get_remote_url()?)
                } else {
                    None
                };

                let rref = if refs {
                    let branch = repo.get_head_branch()?;
                    let gitref = repo.get_head_ref()?;
                    Some(format!("{branch} {gitref}"))
                } else {
                    None
                };

                (url, rref)
            } else {
                (None, None)
            };

            Ok(ListItem { name, url, rref })
        })
        .collect::<Result<Vec<_>>>()?;

    if plugins.is_empty() {
        println!("No plugins installed");
        return Ok(());
    }

    let mut table = Table::new(plugins);

    if !urls {
        table = table.with(tabled::Disable::Column(1..2));
    }

    if !refs {
        table = table.with(tabled::Disable::Column(if urls { 2..3 } else { 1..2 }));
    }

    let table_str = table
        .with(tabled::Style::rounded().vertical_off())
        .with(Modify::new(Segment::all()).with(Alignment::left()))
        .to_string();

    println!("\n{table_str}");

    Ok(())
}

fn display_installed(b: &bool) -> String {
    match b {
        true => "✅".to_owned(),
        false => "".to_owned(),
    }
}

#[derive(Tabled)]
struct ListAllItem {
    name: String,
    url: String,

    #[tabled(display_with = "display_installed")]
    installed: bool,
}

pub fn list_all() -> Result<()> {
    update_registry(DEFAULT_PLUGIN_REGISTRY_URL, DEFAULT_PLUGIN_REGISTRY, false)?;

    let registry_dir = get_dir(REGISTRIES_DIR)?.join(DEFAULT_PLUGIN_REGISTRY);
    let plugins_dir = get_dir(PLUGINS_DIR)?;

    let plugins = fs::read_dir(registry_dir.join("plugins"))?
        .map(|plugin| {
            let plugin = plugin?;
            let name = String::from(plugin.file_name().to_string_lossy());
            let url = parse_short_repo_url(&registry_dir, &name)?;

            let installed_plugin_dir = plugins_dir.join(&name);
            let installed = if installed_plugin_dir.is_dir() {
                let repo = git::GitRepo::new(&installed_plugin_dir)?;
                let remote_url = repo.get_remote_url()?;

                let installed_url = normalize_repo_url(&remote_url);
                let registry_url = normalize_repo_url(&remote_url);
                installed_url == registry_url
            } else {
                false
            };

            Ok(ListAllItem {
                name,
                url,
                installed,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let table = Table::new(plugins)
        .with(tabled::Style::rounded().vertical_off())
        .with(Modify::new(Segment::all()).with(Alignment::left()))
        .to_string();

    println!("\n{table}");

    Ok(())
}

pub fn remove(name: String) -> Result<()> {
    let plugin_dir = get_dir(PLUGINS_DIR)?;
    let remove_plugin_dir = plugin_dir.join(&name);
    if !remove_plugin_dir.is_dir() {
        bail!("plugin `{name}` is not installed");
    }

    fs::remove_dir_all(remove_plugin_dir)?;

    Ok(())
}

pub fn update(name: String, git_ref: Option<String>) -> Result<()> {
    let update_plugin_dir = get_dir(PLUGINS_DIR)?.join(&name);
    if !update_plugin_dir.is_dir() {
        bail!("plugin `{name}` is not installed");
    }

    let repo = git::GitRepo::new(&update_plugin_dir)?;
    if let Some(git_ref) = git_ref {
        println!("updating `{name}` to {git_ref}...");
        repo.update_to_ref(&git_ref)?;
    } else {
        // TODO: Does update without a ref always mean we
        // want to go to the head ref?
        println!("updating `{name}` to latest version...");
        repo.update_to_remote_head()?;
    }

    Ok(())
}

pub fn update_all() -> Result<()> {
    let plugin_dir = get_dir(PLUGINS_DIR)?;

    for plugin in fs::read_dir(plugin_dir)? {
        let plugin = plugin?;

        let name = plugin.file_name();
        let name = name.to_string_lossy();
        println!("updating `{name}`...");

        let repo = git::GitRepo::new(plugin.path())?;

        // TODO: Do we always want to update to the remote head
        // ref here, or skip ones that are pinned?
        repo.update_to_remote_head()?;
    }

    Ok(())
}
