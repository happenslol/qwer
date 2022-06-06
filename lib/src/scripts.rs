use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::versions::Version;

#[derive(Error, Debug)]
pub enum PluginScriptError {
    #[error("script returned a non-0 exit code: {0}")]
    ScriptFailed(String),

    #[error("script `{0}` was not found")]
    ScriptNotFound(String),

    #[error("io error while running script")]
    Io(#[from] std::io::Error),

    #[error("failed to read command output")]
    InvalidOutput(#[from] std::string::FromUtf8Error),

    #[error("version `{0}` for plugin `{1}` was not installed")]
    VersionNotInstalled(String, String),
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
        let mut expr = duct::cmd!(&*script.as_ref().to_string_lossy())
            .stderr_to_stdout()
            .stdout_capture()
            .unchecked();

        for (key, val) in env {
            expr = expr.env(key, val);
        }

        let output = expr.run()?;

        let output_str = String::from_utf8(output.stdout)?;
        if !output.status.success() {
            return Err(PluginScriptError::ScriptFailed(output_str));
        }

        Ok(output_str)
    }

    // Basic functionality

    pub fn list_all(&self) -> Result<Vec<String>, PluginScriptError> {
        Ok(self
            .run_script("bin/list-all", &[])?
            .trim()
            .split(' ')
            .map(|v| v.to_owned())
            .collect())
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
        fs::create_dir_all(&version_download_dir)?;
        fs::create_dir_all(&version_install_dir)?;

        let output = self.run_script(
            &download_script,
            &[
                ("ASDF_INSTALL_TYPE", version.install_type()),
                ("ASDF_INSTALL_VERSION", version_str),
                ("ASDF_INSTALL_PATH", &self.install_dir.to_string_lossy()),
                ("ASDF_DOWNLOAD_PATH", &self.download_dir.to_string_lossy()),
            ],
        )?;

        Ok(output)
    }

    pub fn install(&self, version: &Version) -> Result<String, PluginScriptError> {
        if version == &Version::System {
            return Ok(String::new());
        }

        let install_script = self.plugin_dir.join("bin/install");
        if !install_script.is_file() {
            return Err(PluginScriptError::ScriptNotFound(
                install_script.to_string_lossy().to_string(),
            ));
        }

        // TODO: Escape refs and paths correctly
        let version_str = version.version_str();
        let version_download_dir = self.download_dir.join(version.version_str());
        let version_install_dir = self.install_dir.join(version.version_str());
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

    pub fn uninstall(&self, version: &Version) -> Result<String, PluginScriptError> {
        if version == &Version::System {
            return Ok(String::new());
        }

        let version_str = version.version_str();
        let version_install_dir = self.install_dir.join(version_str);

        if !version_install_dir.is_dir() {
            return Err(PluginScriptError::VersionNotInstalled(
                self.name.clone(),
                version.raw(),
            ));
        }

        let uninstall_script = self.plugin_dir.join("bin/uninstall");
        if !uninstall_script.is_file() {
            fs::remove_dir_all(&version_install_dir)?;
            return Ok(String::new());
        }

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

    // Latest resolution

    pub fn latest_stable(&self) -> Result<&Version, PluginScriptError> {
        todo!()
    }

    // Paths

    pub fn list_bin_paths(&self) -> Result<Vec<String>, PluginScriptError> {
        Ok(Vec::new())
    }

    // Env modification

    pub fn exec_env(&self) -> Result<HashMap<String, String>, PluginScriptError> {
        Ok(HashMap::new())
    }

    pub fn exec_path(&self) -> Result<Vec<String>, PluginScriptError> {
        Ok(Vec::new())
    }

    // Hooks

    pub fn post_plugin_add(&self) -> Result<(), PluginScriptError> {
        Ok(())
    }

    pub fn post_plugin_update(&self) -> Result<(), PluginScriptError> {
        Ok(())
    }

    pub fn pre_plugin_remove(&self) -> Result<(), PluginScriptError> {
        Ok(())
    }

    // Extensions

    pub fn extension(&self, _ext: &str) -> Result<String, PluginScriptError> {
        Ok(String::new())
    }
}
