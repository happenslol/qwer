use std::fs::{self, DirEntry};

use anyhow::{bail, Result};

use crate::dirs::{get_dir, get_plugin_scripts, INSTALLS_DIR};

pub fn installed(name: String, filter: Option<String>) -> Result<()> {
    let install_dir = get_dir(INSTALLS_DIR)?.join(&name);
    if !install_dir.is_dir() {
        bail!("no versions installed for `{name}`");
    }

    let entries = fs::read_dir(&install_dir)?
        .map(|entry| entry)
        .collect::<Result<Vec<DirEntry>, std::io::Error>>()?
        .iter()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let filtered = if let Some(filter) = filter {
        entries
            .into_iter()
            .filter(|version| version.starts_with(&filter))
            .collect()
    } else {
        entries
    };

    for version in filtered {
        println!("{version}");
    }

    Ok(())
}

fn get_filtered_versions(name: String, filter: Option<String>) -> Result<Vec<String>> {
    let scripts = get_plugin_scripts(&name)?;
    let versions = scripts.list_all()?;
    let filtered = if let Some(filter) = filter {
        versions
            .into_iter()
            .filter(|version| version.starts_with(&filter))
            .collect::<Vec<_>>()
    } else {
        versions
    };

    Ok(filtered)
}

pub fn all(name: String, filter: Option<String>) -> Result<()> {
    let versions = get_filtered_versions(name, filter)?;
    if versions.is_empty() {
        bail!("no versions found");
    }

    for version in versions {
        println!("{version}");
    }

    Ok(())
}

pub fn latest(name: String, filter: Option<String>) -> Result<()> {
    let versions = get_filtered_versions(name, filter)?;
    if versions.is_empty() {
        bail!("no versions found");
    }

    println!("{}", versions.last().unwrap());

    Ok(())
}
