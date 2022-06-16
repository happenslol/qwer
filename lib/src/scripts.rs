use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use lazy_static::lazy_static;
use log::trace;
use regex::Regex;
use thiserror::Error;

use crate::{
    env::{Env, IGNORED_ENV_VARS},
    versions::Version,
};

lazy_static! {
    static ref LATEST_STABLE_RE: Regex = Regex::new("-src|-dev|-latest|-stm|[-\\.]rc|-alpha|-beta|[-\\.]pre|-next|(a|b|c)[0-9]+|snapshot|master").unwrap();
}

const ASDF_INSTALL_TYPE: &str = "ASDF_INSTALL_TYPE";
const ASDF_INSTALL_VERSION: &str = "ASDF_INSTALL_VERSION";
const ASDF_INSTALL_PATH: &str = "ASDF_INSTALL_PATH";
const ASDF_DOWNLOAD_PATH: &str = "ASDF_DOWNLOAD_PATH";
const ASDF_CONCURRENCY: &str = "ASDF_CONCURRENCY";
const ASDF_PLUGIN_PATH: &str = "ASDF_PLUGIN_PATH";
const ASDF_PLUGIN_SOURCE_URL: &str = "ASDF_PLUGIN_SOURCE_URL";
const ASDF_PLUGIN_PREV_REF: &str = "ASDF_PLUGIN_PREV_REF";
const ASDF_PLUGIN_POST_REF: &str = "ASDF_PLUGIN_POST_REF";

#[derive(Error, Debug)]
pub enum PluginScriptError {
    #[error("script returned a non-0 exit code:\n{0}")]
    ScriptFailed(String),

    #[error("script `{0}` was not found")]
    ScriptNotFound(String),

    #[error("io error while running script")]
    Io(#[from] std::io::Error),

    #[error("failed to read command output")]
    InvalidOutput(#[from] std::string::FromUtf8Error),

    #[error("version `{version}` for plugin `{plugin}` was not installed")]
    VersionNotInstalled { version: String, plugin: String },

    #[error("version `{version}` for plugin `{plugin}` was already installed")]
    VersionAlreadyInstalled { version: String, plugin: String },

    #[error("no versions were found")]
    NoVersionsFound,

    #[error("no versions were found for query `{0}`")]
    NoMatchingVersionsFound(String),
}

pub struct PluginScripts {
    name: String,
    plugin_dir: PathBuf,
    install_dir: PathBuf,
    download_dir: PathBuf,
}

impl PluginScripts {
    pub fn new<Plugin, Install, Download>(
        name: &str,
        plugins: Plugin,
        installs: Install,
        downloads: Download,
    ) -> Result<Self, PluginScriptError>
    where
        Plugin: AsRef<Path>,
        Install: AsRef<Path>,
        Download: AsRef<Path>,
    {
        let plugin_dir = plugins.as_ref().join(name);
        let install_dir = installs.as_ref().join(name);
        let download_dir = downloads.as_ref().join(name);
        let name = name.to_owned();

        Ok(Self {
            name,
            plugin_dir,
            install_dir,
            download_dir,
        })
    }

    fn run_script<P: AsRef<Path>>(
        &self,
        script: P,
        env: &[(&str, &str)],
    ) -> Result<String, PluginScriptError> {
        if log::log_enabled!(log::Level::Trace) {
            let script_path = script.as_ref();
            let contents = fs::read_to_string(&script_path).unwrap_or_else(|_| "".to_owned());
            trace!("Running script `{script_path:?}` with content: \n{contents}");
        }

        let mut expr = duct::cmd!(script.as_ref())
            .stderr_to_stdout()
            .stdout_capture()
            .unchecked();

        trace!("Setting env for script:\n{env:#?}");

        for (key, val) in env {
            expr = expr.env(key, val);
        }

        let output = expr.run()?;
        let output_str = String::from_utf8(output.stdout)?;
        trace!("Got script output:\n{output_str}");

        if !output.status.success() {
            return Err(PluginScriptError::ScriptFailed(output_str));
        }

        Ok(output_str)
    }

    fn assert_script_exists<P: AsRef<Path>>(&self, script: P) -> Result<(), PluginScriptError> {
        if log::log_enabled!(log::Level::Trace) {
            trace!("Asserting script `{:?}` exists", script.as_ref());
        }

        if !script.as_ref().is_file() {
            return Err(PluginScriptError::ScriptNotFound(
                script.as_ref().to_string_lossy().to_string(),
            ));
        }

        Ok(())
    }

    // Basic functionality

    pub fn list_all(&self) -> Result<Vec<String>, PluginScriptError> {
        let list_all_script = self.plugin_dir.join("bin/list-all");
        self.assert_script_exists(&list_all_script)?;

        Ok(self
            .run_script(&list_all_script, &[])?
            .trim()
            .split(' ')
            .map(|v| v.to_owned())
            .collect())
    }

    pub fn plugin_installed(&self) -> bool {
        self.plugin_dir.is_dir()
    }

    pub fn version_installed(&self, version: &Version) -> bool {
        self.install_dir.join(version.version_str()).is_dir()
    }

    pub fn find_version(&self, version: &str) -> Result<Version, PluginScriptError> {
        let parsed = Version::parse(version);
        match parsed {
            Version::Version(version_str) => {
                let versions = self.list_all()?;

                versions
                    .iter()
                    .find(|raw| &version_str == *raw)
                    .ok_or(PluginScriptError::NoVersionsFound)
                    .map(|raw| Version::parse(raw))
            }
            _ => Ok(parsed),
        }
    }

    pub fn latest(&self) -> Result<Version, PluginScriptError> {
        let versions = self.list_all()?;

        versions
            .last()
            .ok_or(PluginScriptError::NoVersionsFound)
            .map(|raw| Version::parse(raw))
    }

    pub fn has_download(&self) -> bool {
        self.plugin_dir.join("bin/download").is_file()
    }

    pub fn download(&self, version: &Version) -> Result<String, PluginScriptError> {
        if version == &Version::System {
            return Ok(String::new());
        }

        let download_script = self.plugin_dir.join("bin/download");
        if !download_script.is_file() {
            return Ok(String::new());
        }

        // TODO: Escape refs and paths correctly
        let version_str = version.version_str();
        let version_download_dir = self.download_dir.join(version_str);
        let version_install_dir = self.install_dir.join(version_str);
        if version_install_dir.is_dir() {
            return Err(PluginScriptError::VersionAlreadyInstalled {
                plugin: self.name.clone(),
                version: version.raw(),
            });
        }

        fs::create_dir_all(&version_download_dir)?;

        let output = self.run_script(
            &download_script,
            &[
                (ASDF_INSTALL_TYPE, version.install_type()),
                (ASDF_INSTALL_VERSION, version_str),
                (ASDF_INSTALL_PATH, &version_install_dir.to_string_lossy()),
                (ASDF_DOWNLOAD_PATH, &version_download_dir.to_string_lossy()),
            ],
        )?;

        Ok(output)
    }

    pub fn install(
        &self,
        version: &Version,
        concurrency: Option<usize>,
    ) -> Result<String, PluginScriptError> {
        trace!(
            "Installing version {version:?} for plugin `{:?}` to `{:?}`",
            self.plugin_dir,
            self.install_dir,
        );

        if version == &Version::System {
            return Ok(String::new());
        }

        let install_script = self.plugin_dir.join("bin/install");
        self.assert_script_exists(&install_script)?;

        // TODO: Escape refs and paths correctly
        let version_str = version.version_str();
        let version_download_dir = self.download_dir.join(version.version_str());
        let version_install_dir = self.install_dir.join(version.version_str());
        if version_install_dir.is_dir() {
            return Err(PluginScriptError::VersionAlreadyInstalled {
                plugin: self.name.clone(),
                version: version.raw(),
            });
        }

        fs::create_dir_all(&version_install_dir)?;

        let concurrency = concurrency
            .or_else(|| num_threads::num_threads().map(|num| num.get()))
            .unwrap_or(1);

        let output = self.run_script(
            &install_script,
            &[
                (ASDF_INSTALL_TYPE, version.install_type()),
                (ASDF_INSTALL_VERSION, version_str),
                (ASDF_INSTALL_PATH, &version_install_dir.to_string_lossy()),
                (ASDF_DOWNLOAD_PATH, &version_download_dir.to_string_lossy()),
                // TODO: Use num threads by default or accept config
                (ASDF_CONCURRENCY, &concurrency.to_string()),
            ],
        )?;

        // TODO: Allow cleaning download dir

        Ok(output)
    }

    pub fn has_uninstall(&self) -> bool {
        self.plugin_dir.join("bin/uninstall").is_file()
    }

    pub fn rm_version(&self, version: &Version) -> Result<(), PluginScriptError> {
        let version_dir = self.install_dir.join(version.version_str());
        if !version_dir.is_dir() {
            return Ok(());
        }

        Ok(fs::remove_dir_all(&version_dir)?)
    }

    pub fn uninstall(&self, version: &Version) -> Result<String, PluginScriptError> {
        trace!(
            "Uninstalling version {version:?} for plugin `{:?}` from `{:?}`",
            self.plugin_dir,
            self.install_dir,
        );

        if version == &Version::System {
            return Ok(String::new());
        }

        let version_str = version.version_str();
        let version_install_dir = self.install_dir.join(version_str);

        if !version_install_dir.is_dir() {
            return Err(PluginScriptError::VersionNotInstalled {
                plugin: self.name.clone(),
                version: version.raw(),
            });
        }

        let uninstall_script = self.plugin_dir.join("bin/uninstall");
        self.assert_script_exists(&uninstall_script)?;

        let output = self.run_script(
            &uninstall_script,
            &[
                (ASDF_INSTALL_TYPE, version.install_type()),
                (ASDF_INSTALL_VERSION, version_str),
                (ASDF_INSTALL_PATH, &version_install_dir.to_string_lossy()),
            ],
        )?;

        Ok(output)
    }

    // Help strings

    pub fn help_overview(
        &self,
        version: Option<&Version>,
    ) -> Result<Option<String>, PluginScriptError> {
        self.get_help_str("overview", version)
    }

    pub fn help_deps(
        &self,
        version: Option<&Version>,
    ) -> Result<Option<String>, PluginScriptError> {
        self.get_help_str("deps", version)
    }

    pub fn help_config(
        &self,
        version: Option<&Version>,
    ) -> Result<Option<String>, PluginScriptError> {
        self.get_help_str("config", version)
    }

    pub fn help_links(
        &self,
        version: Option<&Version>,
    ) -> Result<Option<String>, PluginScriptError> {
        self.get_help_str("links", version)
    }

    fn get_help_str(
        &self,
        which: &str,
        version: Option<&Version>,
    ) -> Result<Option<String>, PluginScriptError> {
        let script_name = format!("bin/help.{which}");
        let help_path = self.plugin_dir.join(&script_name);

        if !help_path.is_file() {
            return Ok(None);
        }

        let mut env = vec![];
        if let Some(version) = version {
            env.push((ASDF_INSTALL_TYPE, version.install_type()));
            env.push((ASDF_INSTALL_VERSION, version.version_str()));
        }

        let output = self.run_script(&help_path, &env)?;

        Ok(Some(output))
    }

    // Paths

    pub fn list_bin_paths(&self, version: &Version) -> Result<Vec<String>, PluginScriptError> {
        let script_path = self.plugin_dir.join("bin/list-bin-paths");
        if !script_path.is_file() {
            let default_bin_path = self.install_dir.join(version.version_str()).join("bin");
            return Ok(vec![default_bin_path.to_string_lossy().to_string()]);
        }

        let version_dir = self.install_dir.join(version.version_str());
        let output = duct::cmd!(script_path)
            .env(ASDF_INSTALL_TYPE, version.install_type())
            .env(ASDF_INSTALL_VERSION, &version.raw())
            .env(ASDF_INSTALL_PATH, &*version_dir.to_string_lossy())
            .read()?;

        Ok(output
            .trim()
            .split(' ')
            .map(|path| version_dir.join(path).to_string_lossy().to_string())
            .collect())
    }

    // Env modification

    pub fn exec_env(
        &self,
        version: &Version,
    ) -> Result<Option<Vec<(String, String)>>, PluginScriptError> {
        // TODO: Do we need to support adding to path entries here?

        // This is pretty stupid, but there's no way for us to know
        // what vars are being changed unless we actually run the script
        // and compare the env before and after.
        // Surely, there must be a better way to do this...
        let version_dir = self.install_dir.join(version.version_str());
        let exec_path = self.plugin_dir.join("bin/exec-env");
        if !exec_path.is_file() {
            return Ok(None);
        }

        // We use a double newline to delimit input and output, since we can
        // split by that afterwards and take the first and last result.
        // No matter what the script outputs, this will always work since
        // env will always print one line per env var, and escape characters itself.
        let run_str = format!(
            r#"env;echo;{}={} {}={} ={} {}. "{}";echo;env;"#,
            ASDF_INSTALL_TYPE,
            ASDF_INSTALL_VERSION,
            ASDF_INSTALL_PATH,
            version.install_type(),
            version.raw(),
            version_dir.to_string_lossy(),
            exec_path.to_string_lossy(),
        );

        let output = duct::cmd!("bash", "-c", &run_str).read()?;
        let mut parts = output.split("\n\n");
        let env_before = parts
            .next()
            .unwrap_or("")
            .trim()
            .split('\n')
            .collect::<Vec<_>>();

        let env_before_set = HashSet::<&str>::from_iter(env_before);

        let env_after = parts
            .last()
            .unwrap_or("")
            .trim()
            .split('\n')
            .filter(|line| !env_before_set.contains(line))
            .filter_map(|line| line.split_once('='))
            .filter(|(key, _)| !IGNORED_ENV_VARS.contains(key))
            .map(|(key, val)| (key.to_owned(), val.to_owned()))
            .collect::<Vec<_>>();

        Ok(Some(env_after))
    }

    // Latest resolution

    pub fn latest_stable(&self) -> Result<Version, PluginScriptError> {
        let path = self.plugin_dir.join("bin/latest-stable");
        if !path.is_file() {
            let all = self.list_all()?;
            return all
                .iter()
                .filter(|version| !LATEST_STABLE_RE.is_match(version))
                .last()
                .map(|version| Version::parse(version))
                .ok_or_else(|| {
                    PluginScriptError::NoMatchingVersionsFound("latest-stable".to_owned())
                });
        }

        let output = self.run_script(&path, &[])?;
        let version = Version::parse(output.trim());
        Ok(version)
    }

    // Hooks

    pub fn post_plugin_add(&self, install_url: &str) -> Result<(), PluginScriptError> {
        let path = self.plugin_dir.join("bin/post-plugin-add");
        if !path.is_file() {
            return Ok(());
        }

        self.run_script(&path, &[(ASDF_PLUGIN_SOURCE_URL, install_url)])?;

        Ok(())
    }

    pub fn post_plugin_update(&self, prev: &str, post: &str) -> Result<(), PluginScriptError> {
        let path = self.plugin_dir.join("bin/post-plugin-add");
        if !path.is_file() {
            return Ok(());
        }

        self.run_script(
            &path,
            &[
                (ASDF_PLUGIN_PATH, &*self.plugin_dir.to_string_lossy()),
                (ASDF_PLUGIN_PREV_REF, prev),
                (ASDF_PLUGIN_POST_REF, post),
            ],
        )?;

        Ok(())
    }

    pub fn pre_plugin_remove(&self) -> Result<(), PluginScriptError> {
        let path = self.plugin_dir.join("bin/post-plugin-add");
        if !path.is_file() {
            return Ok(());
        }

        self.run_script(
            &path,
            &[(ASDF_PLUGIN_PATH, &*self.plugin_dir.to_string_lossy())],
        )?;

        Ok(())
    }

    // Extensions

    pub fn extension(&self, _ext: &str) -> Result<String, PluginScriptError> {
        Ok(String::new())
    }

    // Helpers

    pub fn get_env(&self, version: &Version) -> Result<Env, PluginScriptError> {
        let mut env = Env::default();

        // first, see if there's an exec-env
        if let Some(exec_env) = self.exec_env(version)? {
            env.vars.extend(exec_env);
        }

        // now, add the bin paths to our path
        env.path.extend(self.list_bin_paths(version)?);

        if env.path.is_empty() {
            let version_path = self.install_dir.join(version.version_str());

            // Check if there's a bin folder in our install
            let maybe_bin_path = version_path.join("bin");
            if maybe_bin_path.is_dir() {
                env.path
                    .insert(maybe_bin_path.to_string_lossy().to_string());
            } else {
                // Just add the install folder
                env.path.insert(version_path.to_string_lossy().to_string());
            }
        }

        Ok(env)
    }

    pub fn resolve(&self, version: &str) -> Result<Version, PluginScriptError> {
        match version {
            "latest" => self.latest(),
            "latest-stable" => self.latest_stable(),
            _ => self.find_version(version),
        }
    }
}
