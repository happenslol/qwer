use anyhow::Result;
use log::trace;
use qwer::{env::Env, versions::Versions};

use crate::dirs::{get_dir, get_plugin_scripts, INSTALLS_DIR, TOOL_VERSIONS};

pub fn update_env() -> Result<()> {
    match get_current_env()? {
        Some(target_env) => {
            let target_env_hash = format!("{}", target_env.hash());
            let current_env_hash = std::env::var("QWER_STATE").ok();
            let changed = current_env_hash
                .as_ref()
                .map(|hash| hash == &target_env_hash)
                .unwrap_or(true);

            trace!(
                "Comparing current env `{target_env_hash}` to target env `{current_env_hash:?}`"
            );

            if !changed {
                trace!("Env did not change");
                return Ok(());
            }

            // Env was changed, update it
            revert_current_env();
            let target_env_str = target_env.serialize();

            // TODO: Store previous env here
            trace!("Setting state to {target_env_hash}");
            std::env::set_var("QWER_STATE", target_env_hash);
            trace!("Setting serialized current env");
            std::env::set_var("QWER_CURRENT", target_env_str);

            for (key, val) in target_env.vars {
                trace!("Setting {key} to {val}");
                std::env::set_var(key, val);
            }

            if !target_env.path.is_empty() {
                let current_path = std::env::var("PATH").unwrap_or_default();
                let target_path = target_env
                    .path
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(":");

                trace!("Adding to path: {:?}", target_env.path);
                std::env::set_var("PATH", format!("{target_path}:{current_path}"));
            }
        }
        None => revert_current_env(),
    };

    Ok(())
}

fn revert_current_env() {
    trace!("Reverting current env");

    // We still might have previous env vars
    // set, so we need to remove them
    let current = std::env::var("QWER_CURRENT").ok();
    // Nothing was set. Clear all vars preemptively and return.
    if current.is_none() {
        clear_state();
        return;
    }

    // Unset the current vars
    let current = Env::deserialize(&current.unwrap()).unwrap_or_default();
    for key in current.vars.keys() {
        std::env::remove_var(key);
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let filtered_path = current_path
        .split(":")
        .filter(|entry| !current.path.contains(*entry))
        .collect::<Vec<_>>()
        .join(":");

    std::env::set_var("PATH", filtered_path);

    // TODO: Restore old vars
    clear_state();
}

fn clear_state() {
    std::env::remove_var("QWER_STATE");
    std::env::remove_var("QWER_PREV");
    std::env::remove_var("QWER_CURRENT");
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
        let install_dir = installs_dir.join(&plugin);
        let found = version_opts.iter().find_map(|version| {
            let path = install_dir.join(version.version_str());
            if path.is_dir() {
                trace!("Version `{version:?}` found at `{path:?}`");
                Some(version)
            } else {
                None
            }
        });

        if found.is_none() {
            trace!("No version found for `{plugin}`");
            continue;
        }

        let scripts = get_plugin_scripts(&plugin)?;
        let version = found.unwrap();
        env.merge(scripts.get_env(version)?);
    }

    if env.vars.is_empty() && env.path.is_empty() {
        Ok(None)
    } else {
        Ok(Some(env))
    }
}
