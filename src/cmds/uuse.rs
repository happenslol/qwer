use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, FuzzySelect, Select};
use threadpool::ThreadPool;

use crate::plugins;

pub fn select_plugin(pool: &ThreadPool) -> Result<String> {
  let available_plugins = plugins::list_all(pool)?;
  let plugin_options = available_plugins
    .iter()
    .map(|plugin| plugin.name.clone())
    .collect::<Vec<_>>();

  let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
    .with_prompt("Choose a plugin")
    .items(&plugin_options)
    .default(0)
    .interact()
    .expect("Failed to select result");

  Ok(String::new())
}

pub fn select_version(pool: &ThreadPool) -> Result<String> {
  Ok(String::new())
}
