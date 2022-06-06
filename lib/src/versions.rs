use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VersionsError {
    #[error("no versions file could be found in the current or any parent directories")]
    NoVersionsFound,

    #[error("the passed workdir was not a directory")]
    InvalidWorkdir,

    #[error("`{0}` is not a valid version entry")]
    InvalidEntry(String),

    #[error("version for `{0}` appeared twice")]
    DuplicateEntry(String),

    #[error("io error while looking for versions file")]
    Io(#[from] io::Error),
}

#[derive(Debug, PartialEq)]
pub enum Version {
    Version(String),
    Ref(String),
    Path(PathBuf),
    System,
}

impl From<PathBuf> for Version {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

pub type Versions = HashMap<String, Vec<Version>>;

/// Walk the directory tree upwards until a file with the given filename is found,
/// and parse it into a versions map. Convenience function that runs
/// `find_versions_file`, reads the found file to string and then runs `parse_versions`
/// on it.
pub fn find_versions<P: AsRef<Path>>(
    workdir: P,
    filename: &str,
) -> Result<Versions, VersionsError> {
    let versions_file_path = find_versions_file(workdir, filename)?;
    let versions_content = fs::read_to_string(versions_file_path)?;
    parse_versions(&versions_content)
}

/// Parse the contents of a version file and return a map of plugin to version.
///
/// # Examples
///
/// ```
/// use qwer::versions::{parse_versions, Version};
///
/// let versions = parse_versions("nodejs 16.0").unwrap();
/// assert_eq!(versions["nodejs"], &[Version::Version("16.0".to_owned())]);
/// ```
pub fn parse_versions(content: &str) -> Result<Versions, VersionsError> {
    let lines = content
        .split('\n')
        .map(|line| line.trim())
        // Filter out comments
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        // Remove comments from line ends, and trim the end
        // again to remove trailing whitespaces
        .map(|line| line.split('#').next().unwrap().trim())
        .collect::<Vec<_>>();

    let mut result = Versions::with_capacity(lines.len());
    for line in lines {
        let parts = line.split(' ').collect::<Vec<_>>();
        if parts.len() <= 1 {
            return Err(VersionsError::InvalidEntry(line.to_owned()));
        }

        if result.contains_key(parts[0]) {
            return Err(VersionsError::DuplicateEntry(parts[0].to_owned()));
        }

        let versions = parts
            .iter()
            .skip(1)
            .map(|version| parse_version(version))
            .collect::<Vec<_>>();

        result.insert(parts[0].to_owned(), versions);
    }

    Ok(result)
}

/// Parse a version string into an enum. This will first try to match `system`, then
/// a `ref`, then a `path` and then fall back to a default `version`. Since the fallback
/// is just using the whole string and pathbufs are not validated, this function does
/// not return an error.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use qwer::versions::{parse_version, Version};
///
/// assert_eq!(parse_version("system"), Version::System);
/// assert_eq!(parse_version("ref:123"), Version::Ref("123".to_owned()));
/// assert_eq!(parse_version("path:/foo"), Version::Path(PathBuf::from("/foo")));
/// assert_eq!(parse_version("1"), Version::Version("1".to_owned()));
/// ```
pub fn parse_version(raw: &str) -> Version {
    if raw == "system" {
        return Version::System;
    }

    if raw.starts_with("ref:") {
        let rref = raw.trim_start_matches("ref:").to_owned();
        return Version::Ref(rref);
    }

    if raw.starts_with("path:") {
        let path_raw = raw.trim_start_matches("path:");
        return PathBuf::from(path_raw).into();
    }

    Version::Version(raw.to_owned())
}

fn find_versions_file<P: AsRef<Path>>(
    workdir: P,
    filename: &str,
) -> Result<PathBuf, VersionsError> {
    let mut current_dir = workdir.as_ref();
    if !current_dir.is_dir() {
        return Err(VersionsError::InvalidWorkdir);
    }

    loop {
        let files = fs::read_dir(&current_dir)?;
        for file in files {
            let file = file?;
            if file.file_name() == filename {
                return Ok(file.path());
            }
        }

        current_dir = current_dir.parent().ok_or(VersionsError::NoVersionsFound)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let to_parse = r#"
# comment
foo 1.2.3
# comment
bar 2.1  # comment
ref ref:123
path path:/foo/bar
system system
multiple 1 ref:123 system
        "#;

        let versions = parse_versions(to_parse).expect("failed to parse versions");

        assert_eq!(versions.len(), 6);
        assert_eq!(versions["foo"], &[Version::Version("1.2.3".to_owned())]);
        assert_eq!(versions["bar"], &[Version::Version("2.1".to_owned())]);
        assert_eq!(versions["ref"], &[Version::Ref("123".to_owned())]);
        assert_eq!(
            versions["path"],
            &[Version::Path(PathBuf::from("/foo/bar"))]
        );
        assert_eq!(versions["system"], &[Version::System]);
        assert_eq!(
            versions["multiple"],
            &[
                Version::Version("1".to_owned()),
                Version::Ref("123".to_owned()),
                Version::System,
            ]
        );
    }

    #[test]
    fn invalid_entries() {
        let invalid = r#"foo1.2.3 # no space"#;
        let result = parse_versions(invalid);
        assert!(matches!(result, Err(VersionsError::InvalidEntry(_))));
    }

    #[test]
    fn duplicate_entries() {
        let invalid = r#"
foo 1.2.3
foo 2.1
        "#;

        let result = parse_versions(invalid);
        assert!(matches!(result, Err(VersionsError::DuplicateEntry(_))));
    }

    #[test]
    fn find_file_same_dir() {
        let workdir = tempfile::tempdir().expect("failed to create temp dir");
        fs::write(workdir.as_ref().join("v"), "foo 1").expect("failed to write versions");

        let versions = find_versions(workdir.as_ref(), "v").expect("failed to find versions");
        assert_eq!(versions["foo"], &[Version::Version("1".to_owned())]);
    }

    #[test]
    fn no_file() {
        let workdir = tempfile::tempdir().expect("failed to create temp dir");
        let subdir = workdir.as_ref().join("foo/bar/baz");
        fs::create_dir_all(&subdir).expect("failed to create dirs");
        let result = find_versions(subdir, "v");
        assert!(matches!(result, Err(VersionsError::NoVersionsFound)));
    }

    #[test]
    fn no_dir() {
        let workdir = tempfile::tempdir().expect("failed to create temp dir");
        let subdir = workdir.as_ref().join("foo/bar/baz");
        let result = find_versions(subdir, "v");
        dbg!(&result);
        assert!(matches!(result, Err(VersionsError::InvalidWorkdir)));
    }

    #[test]
    fn find_file_parent_dir() {
        let workdir = tempfile::tempdir().expect("failed to create temp dir");
        let subdir = workdir.as_ref().join("foo/bar/baz");
        fs::create_dir_all(&subdir).expect("failed to create dirs");
        fs::write(workdir.as_ref().join("v"), "foo 1").expect("failed to write versions");

        let versions = find_versions(subdir, "v").expect("failed to find versions");
        assert_eq!(versions["foo"], &[Version::Version("1".to_owned())]);
    }
}
