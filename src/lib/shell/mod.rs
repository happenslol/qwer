mod bash;
mod zsh;

use std::collections::{HashMap, HashSet};

pub use bash::Bash;
pub use zsh::Zsh;

use super::env::Env;

pub trait Shell {
    fn hook(&self, cmd: &str, hook_fn: &str) -> String;

    fn apply(&self, state: &ShellState) -> String {
        apply_bashlike(state)
    }
}

#[derive(Debug, Default)]
pub struct ShellState {
    add_path: HashSet<String>,
    remove_path: HashSet<String>,
    set_var: HashMap<String, String>,
    unset_var: HashSet<String>,
}

impl ShellState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.unset_var.remove(key);
        self.set_var.insert(key.to_owned(), value.to_owned());
    }

    pub fn unset(&mut self, key: &str) {
        self.set_var.remove(key);
        self.unset_var.insert(key.to_owned());
    }

    pub fn add_path(&mut self, entry: &str) {
        self.remove_path.remove(entry);
        self.add_path.insert(entry.to_owned());
    }

    pub fn remove_path(&mut self, entry: &str) {
        self.add_path.remove(entry);
        self.remove_path.insert(entry.to_owned());
    }

    pub fn apply(&mut self, env: &Env) {
        for (key, val) in &env.vars {
            self.set(key, val);
        }

        for entry in &env.path {
            self.add_path(entry);
        }
    }

    pub fn revert(&mut self, env: &Env) {
        for key in env.vars.keys() {
            self.unset(key);
        }

        for entry in &env.path {
            self.remove_path(entry);
        }
    }
}

pub(crate) fn apply_bashlike(state: &ShellState) -> String {
    let path = std::env::var("PATH").unwrap_or_default();
    let prev_path = path
        .split(':')
        // We filter out both add and remove here, since
        // we want all appended items to be at the front of
        // the new path afterwards.
        .filter(|entry| !state.remove_path.contains(*entry) && !state.add_path.contains(*entry))
        .map(|entry| entry.to_owned());

    let mut new_path = state.add_path.iter().cloned().collect::<Vec<_>>();
    new_path.extend(prev_path);
    let path_str = format!("export PATH={};", new_path.join(":"));

    let unset_str = state
        .unset_var
        .iter()
        // Only unset vars if they are set currently
        .filter(|key| std::env::var(key).is_ok())
        .map(|key| format!("unset {key};"))
        .collect::<Vec<_>>()
        .join("");

    let set_str = state
        .set_var
        .iter()
        .map(|(key, val)| format!("export {key}={val};"))
        .collect::<Vec<_>>()
        .join("");

    format!("{unset_str}{set_str}{path_str}")
}
