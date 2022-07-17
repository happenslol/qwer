use anyhow::{bail, Result};

use crate::{cmds::util::print_help_and_exit, dirs::get_plugin_scripts, versions::Version};

pub fn help(plugin: Option<String>, version: Option<String>) -> Result<()> {
  if plugin.is_none() {
    print_help_and_exit();
  }

  let plugin = plugin.unwrap();
  let scripts = get_plugin_scripts(&plugin)?;
  let version = version.map(|raw| Version::parse(&raw));

  let overview = scripts.help_overview(version.as_ref())?;
  if overview.is_none() {
    bail!("No help for plugin `{plugin}`");
  }

  let overview = overview.unwrap();
  println!("{overview}");

  if let Some(content) = scripts.help_deps(version.as_ref())? {
    println!("{content}");
  }

  if let Some(content) = scripts.help_config(version.as_ref())? {
    println!("{content}");
  }

  if let Some(content) = scripts.help_links(version.as_ref())? {
    println!("{content}");
  }

  Ok(())
}
