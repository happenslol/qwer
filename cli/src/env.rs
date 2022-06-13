use anyhow::Result;
use log::trace;
use qwer::{
    env::Env,
    shell::{Shell, ShellState},
    versions::Versions,
};

use crate::dirs::{get_dir, get_plugin_scripts, INSTALLS_DIR, TOOL_VERSIONS};

pub fn update_env(shell: &dyn Shell) -> Result<String> {
    let mut state = ShellState::new();

    match get_current_env()? {
        Some(target_env) => apply_target_env(shell, &mut state, &target_env),
        None => {
            revert_current_env(shell, &mut state);
            clear_state_vars(shell, &mut state);
        }
    }

    Ok(state.build())
}

fn apply_target_env(shell: &dyn Shell, state: &mut ShellState, target_env: &Env) {
    let target_env_hash = format!("{}", target_env.hash());
    let current_env_hash = std::env::var("QWER_STATE").ok();
    let changed = current_env_hash
        .as_ref()
        .map(|hash| *hash != target_env_hash)
        .unwrap_or(true);

    trace!("Comparing current env `{target_env_hash}` to target env `{current_env_hash:?}`");

    if !changed {
        trace!("Env did not change");
        return;
    }

    // Env was changed, update it
    revert_current_env(shell, state);
    let target_env_str = target_env.serialize();

    let mut stored_env = Env::default();
    for (key, val) in &target_env.vars {
        trace!("Setting {key} to {val}");
        shell.set(state, key, val);

        if let Ok(store_value) = std::env::var(key) {
            if store_value != *val {
                stored_env.vars.insert(key.clone(), store_value);
            }
        }
    }

    trace!("Setting state to {target_env_hash}");
    shell.set(state, "QWER_STATE", &target_env_hash);
    trace!("Setting serialized current env");
    // TODO: We don't actually need the values here. Maybe there
    // should be a different format that only stores keys?
    shell.set(state, "QWER_CURRENT", &target_env_str);

    if !stored_env.vars.is_empty() {
        trace!("Storing changed env values");
        shell.set(state, "QWER_PREV", &stored_env.serialize())
    } else {
        trace!("No env values to store");
        shell.unset(state, "QWER_PREV")
    }

    if !target_env.path.is_empty() {
        let current_path = std::env::var("PATH").unwrap_or_default();
        let target_path = target_env
            .path
            .iter()
            .filter(|entry| !current_path.contains(*entry))
            .cloned()
            .collect::<Vec<_>>()
            .join(":");

        if !target_path.is_empty() {
            trace!("Adding to path: {:?}", target_env.path);
            shell.set(state, "PATH", &format!("{target_path}:{current_path}"));
        }
    }
}

fn revert_current_env(shell: &dyn Shell, state: &mut ShellState) {
    trace!("Reverting current env");

    // We still might have previous env vars
    // set, so we need to remove them
    let current = std::env::var("QWER_CURRENT").ok();

    // Nothing was set
    if current.is_none() {
        return;
    }

    // Unset the current vars
    let current = Env::deserialize(&current.unwrap()).unwrap_or_default();
    for key in current.vars.keys() {
        shell.unset(state, key);
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let filtered_path = current_path
        .split(':')
        .filter(|entry| !current.path.contains(*entry))
        .collect::<Vec<_>>()
        .join(":");

    shell.set(state, "PATH", &filtered_path);

    // TODO: Restore old vars
    if let Ok(prev_env) = std::env::var("QWER_PREV") {
        if let Ok(prev_env) = Env::deserialize(&prev_env) {
            for (key, val) in &prev_env.vars {
                trace!("Restoring var {key}");
                shell.set(state, key, val);
            }
        }
    }
}

fn clear_state_vars(shell: &dyn Shell, state: &mut ShellState) {
    shell.unset(state, "QWER_STATE");
    shell.unset(state, "QWER_PREV");
    shell.unset(state, "QWER_CURRENT");
}

fn get_current_env() -> Result<Option<Env>> {
    trace!("Getting current env");
    let versions_files = Versions::find_all(std::env::current_dir()?, TOOL_VERSIONS)?;
    if versions_files.is_empty() {
        trace!("Empty versions file found");
        return Ok(None);
    }

    let mut versions = Versions::new();
    for mut versions_file in versions_files.into_iter().rev() {
        versions.extend(versions_file.drain());
    }

    let installs_dir = get_dir(INSTALLS_DIR)?;
    let mut env = Env::default();

    for (plugin, version_opts) in versions.iter() {
        trace!("Finding version for plugin `{plugin}`, options: `{version_opts:?}`");
        let install_dir = installs_dir.join(plugin);
        let found = version_opts
            .iter()
            .find(|version| install_dir.join(version.version_str()).is_dir());

        if found.is_none() {
            trace!("No version found for `{plugin}`");
            continue;
        }

        trace!("Version `{found:?}` found");
        let scripts = get_plugin_scripts(plugin)?;
        let version = found.unwrap();
        env.merge(scripts.get_env(version)?);
    }

    if env.vars.is_empty() && env.path.is_empty() {
        Ok(None)
    } else {
        Ok(Some(env))
    }
}
