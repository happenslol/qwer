use anyhow::Result;
use log::trace;
use qwer::{
    versions::{Versions, VersionsError},
    Env,
};

use crate::dirs::{get_dir, get_plugin_scripts, INSTALLS_DIR, TOOL_VERSIONS};

pub fn get_current_env() -> Result<Option<Env>> {
    trace!("Getting current env");
    let versions = Versions::find_any(std::env::current_dir()?, TOOL_VERSIONS);
    if let Err(VersionsError::NoVersionsFound) = versions {
        trace!("No versions file found");
        return Ok(None);
    }

    let versions = versions?;
    if versions.is_empty() {
        trace!("Empty versions file found");
        return Ok(None);
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

    Ok(Some(env))
}
