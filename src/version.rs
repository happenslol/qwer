use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};

use crate::{
    dirs::{get_plugin_scripts, TOOL_VERSIONS},
    lib::{
        shell::{Bash, Shell, ShellState},
        versions::Versions,
    },
};

fn use_version_for_dir(name: String, version: String, path: PathBuf) -> Result<()> {
    let scripts = get_plugin_scripts(&name)?;
    let version = scripts.resolve(&version)?;
    if !scripts.version_installed(&version) {
        bail!(
            "Version `{}` is not installed for plugin `{}`",
            version.raw(),
            &name
        );
    }

    let global_versions_path = path.join(TOOL_VERSIONS);
    let mut versions = if global_versions_path.is_file() {
        Versions::find(&path, TOOL_VERSIONS)?
    } else {
        Versions::new()
    };

    versions.insert(name, vec![version]);
    versions.save(&global_versions_path)?;

    Ok(())
}

pub fn global(name: String, version: String) -> Result<()> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Failed to get home dir"))?;
    use_version_for_dir(name, version, home_dir)
}

pub fn local(name: String, version: String) -> Result<()> {
    use_version_for_dir(name, version, std::env::current_dir()?)
}

pub fn shell(name: String, version: String) -> Result<()> {
    let scripts = get_plugin_scripts(&name)?;

    let version = scripts.resolve(&version)?;
    if !scripts.version_installed(&version) {
        bail!(
            "Version `{}` is not installed for plugin `{}`",
            version.raw(),
            &name
        );
    }

    let env = scripts.get_env(&version)?;
    let mut state = ShellState::new();

    for (key, val) in &env.vars {
        state.set(key, val);
    }

    for entry in &env.path {
        state.add_path(entry);
    }

    let set_env = Bash.apply(&state);
    print!("#!/bin/env bash\n{set_env}");

    Ok(())
}
