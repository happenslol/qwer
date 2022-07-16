use log::trace;
use std::{
    collections::HashMap,
    fs, io,
    ops::{Deref, DerefMut},
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

#[derive(Debug, Clone, PartialEq)]
pub enum Version {
    Version(String),
    Ref(String),
    Path(String),
    System,
}

impl Version {
    /// Parse a version string into an enum. This will first try to match `system`, then
    /// a `ref`, then a `path` and then fall back to a default `version`. Since the fallback
    /// is just using the whole string and pathbufs are not validated, this function does
    /// not return an error.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use qwer::versions::Version;
    ///
    /// assert_eq!(Version::parse("system"), Version::System);
    /// assert_eq!(Version::parse("ref:123"), Version::Ref("123".to_owned()));
    /// assert_eq!(Version::parse("path:/foo"), Version::Path("/foo".to_owned()));
    /// assert_eq!(Version::parse("1"), Version::Version("1".to_owned()));
    /// ```
    pub fn parse(raw: &str) -> Self {
        trace!("Parsing version string {raw}");

        if raw == "system" {
            return Version::System;
        }

        if raw.starts_with("ref:") {
            let rref = raw.trim_start_matches("ref:").to_owned();
            return Version::Ref(rref);
        }

        if raw.starts_with("path:") {
            let path = raw.trim_start_matches("path:").to_owned();
            return Version::Path(path);
        }

        Version::Version(raw.to_owned())
    }

    pub fn install_type(&self) -> &'static str {
        match self {
            Self::Version(_) => "version",
            Self::Ref(_) => "ref",
            Self::Path(_) => "path",
            Self::System => "system",
        }
    }

    pub fn version_str(&self) -> &str {
        match self {
            Self::Version(version) => version,
            Self::Ref(rref) => rref,
            Self::Path(path) => path,
            Self::System => "",
        }
    }

    pub fn raw(&self) -> String {
        match self {
            Self::Version(version) => version.to_owned(),
            Self::Ref(rref) => format!("ref:{rref}"),
            Self::Path(path) => format!("path:{path}"),
            Self::System => "system".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Versions(HashMap<String, Vec<Version>>);

impl Versions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse the contents of a version file and return a map of plugin to version.
    ///
    /// # Examples
    ///
    /// ```
    /// use qwer::versions::{Version, Versions};
    ///
    /// let versions = Versions::parse("nodejs 16.0").unwrap();
    /// assert_eq!(versions["nodejs"], &[Version::Version("16.0".to_owned())]);
    /// ```
    pub fn parse(content: &str) -> Result<Self, VersionsError> {
        trace!("Parsing versions:\n{content}");

        let lines = content
            .split('\n')
            .map(|line| line.trim())
            // Filter out comments
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            // Remove comments from line ends, and trim the end
            // again to remove trailing whitespaces
            .map(|line| line.split('#').next().unwrap().trim())
            .collect::<Vec<_>>();

        let mut result = Versions(HashMap::with_capacity(lines.len()));
        for line in lines {
            let parts = line.split(' ').collect::<Vec<_>>();
            if parts.len() <= 1 {
                return Err(VersionsError::InvalidEntry(line.to_owned()));
            }

            if result.0.contains_key(parts[0]) {
                return Err(VersionsError::DuplicateEntry(parts[0].to_owned()));
            }

            let versions = parts
                .iter()
                .skip(1)
                .map(|version| Version::parse(version))
                .collect::<Vec<_>>();

            result.0.insert(parts[0].to_owned(), versions);
        }

        Ok(result)
    }

    /// Find a file in the local directory and parse it into a versions map.
    /// and parse it into a versions map.
    pub fn find<P: AsRef<Path>>(workdir: P, filename: &str) -> Result<Self, VersionsError> {
        let versions_file_path = workdir.as_ref().join(filename);
        trace!("Looking for versions file at `{:?}`", versions_file_path);
        let versions_content = fs::read_to_string(versions_file_path)?;
        Self::parse(&versions_content)
    }

    /// Walk the directory tree upwards until a file with the given filename is found,
    /// and parse it into a versions map.
    pub fn find_any<P: AsRef<Path>>(workdir: P, filename: &str) -> Result<Self, VersionsError> {
        let versions_file_path = find_versions_file(workdir, filename)?;
        let versions_content = fs::read_to_string(versions_file_path)?;
        Self::parse(&versions_content)
    }

    /// Continually walk the directory tree upwards and find all version files, parsing
    /// all of them into version maps. The returned results will be in the order the
    /// files were found in.
    pub fn find_all<P: AsRef<Path>>(
        workdir: P,
        filename: &str,
    ) -> Result<Vec<Self>, VersionsError> {
        let versions_file_paths = find_all_versions_files(workdir, filename)?;

        versions_file_paths
            .iter()
            .map(fs::read_to_string)
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .map(|content| Self::parse(content))
            .collect()
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), VersionsError> {
        let contents = self
            .iter()
            .map(|entry| {
                format!(
                    "{} {}",
                    entry.0,
                    entry
                        .1
                        .iter()
                        .map(Version::raw)
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(path, contents)?;
        Ok(())
    }
}

impl Deref for Versions {
    type Target = HashMap<String, Vec<Version>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Versions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn find_all_versions_files<P: AsRef<Path>>(
    workdir: P,
    filename: &str,
) -> Result<Vec<PathBuf>, VersionsError> {
    let mut current_dir = workdir.as_ref();
    if !current_dir.is_dir() {
        return Err(VersionsError::InvalidWorkdir);
    }

    let mut result = Vec::new();
    loop {
        trace!("Looking for versions file in {:?}", current_dir);

        let files = fs::read_dir(&current_dir)?;
        for file in files {
            let file = file?;
            if file.file_name() == filename {
                result.push(file.path());
            }
        }

        let next_dir = current_dir.parent();
        if next_dir.is_none() {
            break;
        }

        current_dir = next_dir.unwrap();
    }

    Ok(result)
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
        trace!("Looking for versions file in {:?}", current_dir);

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

        let versions = Versions::parse(to_parse).expect("failed to parse versions");

        assert_eq!(versions.len(), 6);
        assert_eq!(versions["foo"], &[Version::Version("1.2.3".to_owned())]);
        assert_eq!(versions["bar"], &[Version::Version("2.1".to_owned())]);
        assert_eq!(versions["ref"], &[Version::Ref("123".to_owned())]);
        assert_eq!(versions["path"], &[Version::Path("/foo/bar".to_owned())]);
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
        let result = Versions::parse(invalid);
        assert!(matches!(result, Err(VersionsError::InvalidEntry(_))));
    }

    #[test]
    fn duplicate_entries() {
        let invalid = r#"
foo 1.2.3
foo 2.1
        "#;

        let result = Versions::parse(invalid);
        assert!(matches!(result, Err(VersionsError::DuplicateEntry(_))));
    }

    #[test]
    fn find_file_same_dir() {
        let workdir = tempfile::tempdir().expect("failed to create temp dir");
        fs::write(workdir.as_ref().join("v"), "foo 1").expect("failed to write versions");

        let versions = Versions::find_any(workdir.as_ref(), "v").expect("failed to find versions");
        assert_eq!(versions["foo"], &[Version::Version("1".to_owned())]);
    }

    #[test]
    fn no_file() {
        let workdir = tempfile::tempdir().expect("failed to create temp dir");
        let subdir = workdir.as_ref().join("foo/bar/baz");
        fs::create_dir_all(&subdir).expect("failed to create dirs");
        let result = Versions::find_any(subdir, "v");
        assert!(matches!(result, Err(VersionsError::NoVersionsFound)));
    }

    #[test]
    fn no_dir() {
        let workdir = tempfile::tempdir().expect("failed to create temp dir");
        let subdir = workdir.as_ref().join("foo/bar/baz");
        let result = Versions::find_any(subdir, "v");
        assert!(matches!(result, Err(VersionsError::InvalidWorkdir)));
    }

    #[test]
    fn find_file_parent_dir() {
        let workdir = tempfile::tempdir().expect("failed to create temp dir");
        let subdir = workdir.as_ref().join("foo/bar/baz");
        fs::create_dir_all(&subdir).expect("failed to create dirs");
        fs::write(workdir.as_ref().join("v"), "foo 1").expect("failed to write versions");

        let versions = Versions::find_any(subdir, "v").expect("failed to find versions");
        assert_eq!(versions["foo"], &[Version::Version("1".to_owned())]);
    }
}
