use std::{
    fs,
    path::{Path, PathBuf},
};

use log::trace;
use thiserror::Error;

use crate::{versions::Version, Env};

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
            let contents = fs::read_to_string(&script_path).unwrap_or("".to_owned());
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
                ("ASDF_INSTALL_TYPE", version.install_type()),
                ("ASDF_INSTALL_VERSION", version_str),
                ("ASDF_INSTALL_PATH", &version_install_dir.to_string_lossy()),
                (
                    "ASDF_DOWNLOAD_PATH",
                    &version_download_dir.to_string_lossy(),
                ),
            ],
        )?;

        Ok(output)
    }

    pub fn install(&self, version: &Version) -> Result<String, PluginScriptError> {
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

        let output = self.run_script(
            &install_script,
            &[
                ("ASDF_INSTALL_TYPE", version.install_type()),
                ("ASDF_INSTALL_VERSION", version_str),
                ("ASDF_INSTALL_PATH", &version_install_dir.to_string_lossy()),
                (
                    "ASDF_DOWNLOAD_PATH",
                    &version_download_dir.to_string_lossy(),
                ),
                // TODO: Use num threads by default or accept config
                ("ASDF_CONCURRENCY", "1"),
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
                ("ASDF_INSTALL_TYPE", version.install_type()),
                ("ASDF_INSTALL_VERSION", version_str),
                ("ASDF_INSTALL_PATH", &version_install_dir.to_string_lossy()),
            ],
        )?;

        Ok(output)
    }

    // Help strings

    pub fn help_overview(&self, version: Option<&Version>) -> Result<String, PluginScriptError> {
        self.get_help_str("overview", version)
    }

    pub fn help_deps(&self, version: Option<&Version>) -> Result<String, PluginScriptError> {
        self.get_help_str("deps", version)
    }

    pub fn help_config(&self, version: Option<&Version>) -> Result<String, PluginScriptError> {
        self.get_help_str("config", version)
    }

    pub fn help_links(&self, version: Option<&Version>) -> Result<String, PluginScriptError> {
        self.get_help_str("links", version)
    }

    fn get_help_str(
        &self,
        which: &str,
        version: Option<&Version>,
    ) -> Result<String, PluginScriptError> {
        let script_name = format!("help.{which}");
        let help_path = self.plugin_dir.join(&script_name);

        if !help_path.is_file() {
            return Err(PluginScriptError::ScriptNotFound(script_name));
        }

        let mut env = vec![];
        if let Some(version) = version {
            env.push(("ASDF_INSTALL_TYPE", version.install_type()));
            env.push(("ASDF_INSTALL_VERSION", version.version_str()));
        }

        let output = self.run_script(&help_path, &env)?;

        Ok(output)
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
            .env("ASDF_INSTALL_TYPE", version.install_type())
            .env("ASDF_INSTALL_VERSION", &version.raw())
            .env("ASDF_INSTALL_PATH", &*version_dir.to_string_lossy())
            .read()?;

        Ok(output
            .trim()
            .split(' ')
            .map(|path| version_dir.join(path).to_string_lossy().to_string())
            .collect())
    }

    // Env modification

    pub fn exec_env(&self, version: &Version) -> Option<String> {
        let version_dir = self.install_dir.join(version.version_str());
        let exec_path = self.plugin_dir.join("bin/exec-env");
        if !exec_path.is_file() {
            return None;
        }

        let run_str = format!(
            r#"ASDF_INSTALL_TYPE={} ASDF_INSTALL_VERSION={} ASDF_INSTALL_PATH={} . "{}""#,
            version.install_type(),
            version.raw(),
            version_dir.to_string_lossy(),
            exec_path.to_string_lossy(),
        );

        Some(run_str)
    }

    pub fn exec_path(&self, _version: &Version) -> Result<Vec<String>, PluginScriptError> {
        todo!()
    }

    // Latest resolution

    pub fn latest_stable(&self) -> Result<Version, PluginScriptError> {
        todo!()
    }

    // Hooks

    pub fn post_plugin_add(&self) -> Result<(), PluginScriptError> {
        todo!()
    }

    pub fn post_plugin_update(&self) -> Result<(), PluginScriptError> {
        todo!()
    }

    pub fn pre_plugin_remove(&self) -> Result<(), PluginScriptError> {
        todo!()
    }

    // Extensions

    pub fn extension(&self, _ext: &str) -> Result<String, PluginScriptError> {
        Ok(String::new())
    }

    // Helpers

    pub fn get_env(&self, version: &Version) -> Result<Env, PluginScriptError> {
        let mut env = Env::default();

        // first, see if there's an exec-env
        if let Some(exec_env_run) = self.exec_env(&version) {
            env.run.push(exec_env_run);
        }

        // now, add the bin paths to our path
        env.path.extend(self.list_bin_paths(&version)?);

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
