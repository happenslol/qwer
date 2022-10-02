use console::{style, StyledObject};

pub fn plugin_version(plug: &str, ver: &str) -> String {
  format!("{} {}", plugin(plug), version(ver))
}

pub fn plugin(name: &str) -> StyledObject<&str> {
  style(name).bold().blue()
}

pub fn version(name: &str) -> StyledObject<&str> {
  style(name).bold().cyan()
}

pub fn _registry(name: &str) -> StyledObject<&str> {
  style(name).bold().yellow()
}
