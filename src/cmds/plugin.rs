use std::fs;

use anyhow::{bail, Result};
use console::style;
use tabled::{object::Segment, Alignment, Modify, Table, Tabled};

use crate::{
  dirs::{get_dir, get_plugin_scripts, INSTALLS_DIR, PLUGINS_DIR},
  git,
  plugins,
};

fn display_option(opt: &Option<String>) -> String {
  match opt {
    Some(s) => s.clone(),
    None => String::new(),
  }
}

pub fn add(name: String, git_url: Option<String>) -> Result<()> {
  plugins::add(name, git_url)
}

#[derive(Tabled)]
struct ListItem {
  name: String,

  #[tabled(display_with = "display_option")]
  url: Option<String>,

  #[tabled(rename = "ref", display_with = "display_option")]
  rref: Option<String>,
}

pub fn list(force_refresh: bool, urls: bool, refs: bool) -> Result<()> {
  let plugins = plugins::list(force_refresh)?;
  if plugins.is_empty() {
    println!("No plugins installed");
    return Ok(());
  }

  let plugin_items = plugins.into_iter().map(|entry| ListItem {
    name: entry.name,
    url: if urls {
      Some(style(entry.url).dim().to_string())
    } else {
      None
    },
    rref: if refs {
      Some(style(entry.rref).cyan().to_string())
    } else {
      None
    },
  });

  let mut table = Table::new(plugin_items);

  if !urls {
    table = table.with(tabled::Disable::Column(1..2));
  }

  if !refs {
    table = table.with(tabled::Disable::Column(if urls { 2..3 } else { 1..2 }));
  }

  let table_str = table
    .with(tabled::Style::blank())
    .with(Modify::new(Segment::all()).with(Alignment::left()))
    .to_string();

  println!("\n{table_str}");
  Ok(())
}

#[derive(Tabled)]
struct ListAllItem {
  name: String,
  url: String,
}

pub fn list_all(force_refresh: bool) -> Result<()> {
  let plugins = plugins::list_all(force_refresh)?;

  let plugin_items = plugins.into_iter().map(|entry| ListAllItem {
    name: entry.name,
    url: style(entry.url).dim().to_string(),
  });

  let table = Table::new(plugin_items)
    .with(tabled::Style::blank().vertical_off())
    .with(Modify::new(Segment::all()).with(Alignment::left()))
    .to_string();

  println!("\n{table}");
  Ok(())
}

pub fn remove(name: String) -> Result<()> {
  let plugin_dir = get_dir(PLUGINS_DIR)?;
  let remove_plugin_dir = plugin_dir.join(&name);
  if !remove_plugin_dir.is_dir() {
    bail!("Plugin {} is not installed", style(&name).bold());
  }

  let scripts = get_plugin_scripts(&name)?;
  scripts.pre_plugin_remove()?;

  fs::remove_dir_all(remove_plugin_dir)?;

  let install_dir = get_dir(INSTALLS_DIR)?.join(&name);
  if install_dir.is_dir() {
    fs::remove_dir_all(install_dir)?;
  }

  Ok(())
}

pub fn update(name: String, git_ref: Option<String>) -> Result<()> {
  let update_plugin_dir = get_dir(PLUGINS_DIR)?.join(&name);
  if !update_plugin_dir.is_dir() {
    bail!("Plugin {} is not installed", style(&name).bold());
  }

  let repo = git::GitRepo::new(&update_plugin_dir)?;
  let prev = repo.get_head_ref()?;

  if let Some(git_ref) = git_ref {
    println!("");
    repo.update_to_ref(
      &git_ref,
      Some(&format!(
        "Updating plugin {} to version {}",
        style(&name).bold(),
        style(&git_ref).bold(),
      )),
    )?;
  } else {
    // TODO: Does update without a ref always mean we
    // want to go to the head ref?
    repo.update_to_remote_head(
      Some(&format!(
        "Finding latest version for plugin {}",
        style(&name).bold()
      )),
      Some(&format!(
        "Updating plugin {} to latest version",
        style(&name).bold()
      )),
    )?;
  }

  let scripts = get_plugin_scripts(&name)?;
  let post = repo.get_head_ref()?;
  scripts.post_plugin_update(&prev, &post)?;

  Ok(())
}

pub fn update_all() -> Result<()> {
  let plugin_dir = get_dir(PLUGINS_DIR)?;
  let dirs = fs::read_dir(plugin_dir)?.collect::<Vec<_>>();
  let mut repos = Vec::with_capacity(dirs.len());

  for plugin in dirs {
    let plugin = plugin?;

    let name = plugin.file_name();
    let name = name.to_string_lossy().to_string();

    let repo = git::GitRepo::new(plugin.path())?;
    repos.push((name, repo));
  }

  // TODO: This is janky as hell
  // pool.set_num_threads(repos.len());
  // for (name, repo) in repos {
  //   pool.execute(move || {
  //     let pool = ThreadPool::new(1);
  //
  //     // TODO: Do we always want to update to the remote head
  //     // ref here, or skip ones that are pinned?
  //     repo.update_to_remote_head(
  //       Some(&format!(
  //         "Finding remote head branch for plugin {}",
  //         style(&name).bold()
  //       )),
  //       Some(&format!(
  //         "Updating plugin {} to latest version",
  //         style(&name).bold()
  //       )),
  //     );
  //   });
  // }
  //
  // pool.join();
  Ok(())
}
