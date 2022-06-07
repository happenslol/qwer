use anyhow::Result;
use qwer::{
    versions::{Versions, VersionsError},
    Env,
};

use crate::dirs::{get_dir, get_plugin_scripts, INSTALLS_DIR, TOOL_VERSIONS};

pub fn get_current_env() -> Result<Option<Env>> {
    let versions = Versions::find_any(std::env::current_dir()?, TOOL_VERSIONS);
    if let Err(VersionsError::NoVersionsFound) = versions {
        return Ok(None);
    }

    let versions = versions?;
    if versions.is_empty() {
        return Ok(None);
    }

    let installs_dir = get_dir(INSTALLS_DIR)?;
    let mut env = Env::default();

    for (plugin, version_opts) in versions.iter() {
        let install_dir = installs_dir.join(&plugin);
        let found = version_opts.iter().find_map(|version| {
            let path = install_dir.join(version.version_str());
            if path.is_dir() {
                Some(version)
            } else {
                None
            }
        });

        if found.is_none() {
            continue;
        }

        let scripts = get_plugin_scripts(&plugin)?;
        let version = found.unwrap();

        // first, see if there's an exec-env
        if let Some(exec_env_run) = scripts.exec_env(&version) {
            env.run.push(exec_env_run);
        }

        // now, add the bin paths to our path
        env.path.extend(scripts.list_bin_paths(&version)?);
    }

    Ok(Some(env))
}
