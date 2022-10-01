use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};

use crate::plugins;

pub fn select_plugin() -> Result<String> {
  let available_plugins = plugins::list_all(false)?;
  let plugin_options = available_plugins
    .iter()
    .map(|plugin| plugin.name.clone())
    .collect::<Vec<_>>();

  let _selection = FuzzySelect::with_theme(&ColorfulTheme::default())
    .with_prompt("Choose a plugin")
    .items(&plugin_options)
    .default(0)
    .interact()
    .expect("Failed to select result");

  Ok(String::new())
}

pub fn select_version() -> Result<String> {
  Ok(String::new())
}
