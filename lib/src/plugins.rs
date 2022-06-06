use std::{fs, path::Path};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShortPluginError {
    #[error("plugin `{0}` was not found in the plugin repo")]
    NotFound(String),

    #[error("io error while looking for plugin")]
    Io(#[from] std::io::Error),

    #[error("plugin shortcut `{0}` should be in format `repository = <git-url>`")]
    InvalidFile(String),
}

/// Retrieve the repository url from a directory containing plugin references.
/// See [the asdf plugin repository](https://github.com/asdf-vm/asdf-plugins/tree/master/plugins)
/// for the expected file format and contents.
pub fn parse_short_repo_url<P: AsRef<Path>>(
    registry: P,
    plugin: &str,
) -> Result<String, ShortPluginError> {
    let plugin_file = registry.as_ref().join("plugins").join(plugin);
    if !plugin_file.is_file() {
        return Err(ShortPluginError::NotFound(plugin.to_owned()));
    }

    let contents = fs::read_to_string(plugin_file)?;
    let parts = contents.split('=').collect::<Vec<&str>>();
    if parts.len() != 2 || parts[0].trim() != "repository" {
        return Err(ShortPluginError::InvalidFile(contents));
    }

    Ok(parts[1].trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_short() {
        let workdir = tempfile::tempdir().expect("failed to create temp dir");
        fs::write(workdir.as_ref().join("foo"), "repository = bar")
            .expect("failed to write plugin file");

        let result = parse_short_repo_url(&workdir, "foo").expect("failed to parse");
        assert_eq!(result, "bar");
    }

    #[test]
    fn parse_not_found() {
        let workdir = tempfile::tempdir().expect("failed to create temp dir");
        fs::write(workdir.as_ref().join("foo"), "repository = bar")
            .expect("failed to write plugin file");

        let result = parse_short_repo_url(&workdir, "bar");
        assert!(matches!(result, Err(ShortPluginError::NotFound(_))));
    }

    #[test]
    fn parse_invalid_format() {
        let workdir = tempfile::tempdir().expect("failed to create temp dir");
        fs::write(workdir.as_ref().join("foo"), "invalid format")
            .expect("failed to write plugin file");

        let result = parse_short_repo_url(&workdir, "foo");
        assert!(matches!(result, Err(ShortPluginError::InvalidFile(_))));
    }
}
