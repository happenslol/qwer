use anyhow::Result;
use log::trace;
use qwer::{
    env::Env,
    shell::ShellState,
    versions::{Version, Versions},
};

use crate::dirs::{get_dir, get_plugin_scripts, INSTALLS_DIR, TOOL_VERSIONS};

const QWER_STATE: &str = "QWER_STATE";
const QWER_PREV: &str = "QWER_PREV";
const QWER_CURRENT: &str = "QWER_CURRENT";

pub fn update_env() -> Result<ShellState> {
    let mut state = ShellState::new();

    match get_target_env()? {
        Some(target_env) => apply_target_env(&mut state, &target_env),
        None => {
            revert_current_env(&mut state);
            clear_state_vars(&mut state);
        }
    }

    Ok(state)
}

fn apply_target_env(state: &mut ShellState, target_env: &Env) {
    let target_env_hash = format!("{}", target_env.hash());
    let current_env_hash = std::env::var(QWER_STATE).ok();
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
    revert_current_env(state);
    let target_env_str = target_env.serialize();

    let mut stored_env = Env::default();
    for (key, val) in &target_env.vars {
        trace!("Setting {key} to {val}");
        state.set(key, val);

        if let Ok(store_value) = std::env::var(key) {
            if store_value != *val {
                stored_env.vars.insert(key.clone(), store_value);
            }
        }
    }

    state.set(QWER_STATE, &target_env_hash);
    trace!("Setting state to {target_env_hash}");

    // TODO: We don't actually need the values here. Maybe there
    // should be a different format that only stores keys?
    state.set(QWER_CURRENT, &target_env_str);
    trace!("Setting serialized current env");

    if !stored_env.vars.is_empty() {
        trace!("Storing changed env values");
        state.set(QWER_PREV, &stored_env.serialize())
    } else {
        trace!("No env values to store");
        state.unset(QWER_PREV)
    }

    for entry in &target_env.path {
        state.add_path(entry);
    }
}

fn revert_current_env(state: &mut ShellState) {
    trace!("Reverting current env");

    // We still might have previous env vars
    // set, so we need to remove them
    let current = std::env::var(QWER_CURRENT).ok();

    // Nothing was set
    if current.is_none() {
        trace!("No current env found");
        return;
    }

    // Unset the current vars
    let current = Env::deserialize(&current.unwrap()).unwrap_or_default();
    state.revert(&current);

    if let Ok(prev_env) = std::env::var(QWER_PREV) {
        if let Ok(prev_env) = Env::deserialize(&prev_env) {
            state.apply(&prev_env);
        }
    }
}

fn clear_state_vars(state: &mut ShellState) {
    state.unset(QWER_STATE);
    state.unset(QWER_PREV);
    state.unset(QWER_CURRENT);
}

fn get_target_env() -> Result<Option<Env>> {
    trace!("Getting current env");
    let versions = get_combined_versions()?;
    if versions.is_none() {
        return Ok(None);
    }

    let versions = versions.unwrap();
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

pub fn current(name: String) -> Result<()> {
    if let Some(current) = find_current_version(&name)? {
        println!("{} {}", name, current.raw());
    } else {
        println!("No version in use for {}", name);
    }

    Ok(())
}

pub fn wwhere(name: String, version: Option<String>) -> Result<()> {
    Ok(())
}

fn find_current_version(name: &str) -> Result<Option<Version>> {
    let versions = get_combined_versions()?;
    if versions.is_none() {
        return Ok(None);
    }

    let versions = versions.unwrap();
    let found_plugin = versions.get(name);
    if found_plugin.is_none() {
        return Ok(None);
    }

    let found_plugin = found_plugin.unwrap();
    let installs_dir = get_dir(INSTALLS_DIR)?;
    let install_dir = installs_dir.join(name);

    let found_install = found_plugin
        .iter()
        .find(|version| install_dir.join(version.version_str()).is_dir());

    Ok(found_install.map(|found| found.to_owned()))
}

fn get_combined_versions() -> Result<Option<Versions>> {
    let versions_files = Versions::find_all(std::env::current_dir()?, TOOL_VERSIONS)?;
    if versions_files.is_empty() {
        trace!("Empty versions file found");
        return Ok(None);
    }

    let mut versions = Versions::new();
    for mut versions_file in versions_files.into_iter().rev() {
        versions.extend(versions_file.drain());
    }

    Ok(Some(versions))
}
