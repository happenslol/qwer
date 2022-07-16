use std::{
    collections::{HashMap, HashSet},
    fs,
    os::unix::prelude::PermissionsExt,
    path::{Path, PathBuf},
};

use lazy_static::lazy_static;
use log::trace;
use regex::Regex;
use thiserror::Error;
use threadpool::ThreadPool;

use crate::{
    lib::{
        env::{Env, IGNORED_ENV_VARS},
        versions::Version,
    },
    prog::CmdContext,
};

lazy_static! {
    static ref LATEST_STABLE_RE: Regex = Regex::new("-src|-dev|-latest|-stm|[-\\.]rc|-alpha|-beta|[-\\.]pre|-next|(a|b|c)[0-9]+|snapshot|master").unwrap();
    static ref EXPORT_ECHO_RE: Regex = Regex::new("export ").unwrap();
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

    #[error("error while running command")]
    CommandError(#[from] crate::prog::CmdError),
}

pub struct PluginScripts {
    name: String,
    plugin_dir: PathBuf,
    install_dir: PathBuf,
    download_dir: PathBuf,
    script_env_path: String,
}

impl PluginScripts {
    pub fn new<Plugin, Install, Download>(
        name: &str,
        plugins: Plugin,
        installs: Install,
        downloads: Download,
        extra_path: &[&str],
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

        let mut script_env_path = extra_path
            .iter()
            .map(|entry| entry.to_string())
            .collect::<Vec<String>>();

        if let Ok(current_path) = std::env::var("PATH") {
            script_env_path.push(current_path);
        }

        let script_env_path = script_env_path.join(":");

        Ok(Self {
            name,
            plugin_dir,
            install_dir,
            download_dir,
            script_env_path,
        })
    }

    fn run_script<P: AsRef<Path>, T: 'static + Send>(
        &self,
        pool: &ThreadPool,
        parse_output: fn(String) -> T,
        script: P,
        env: &[(&str, &str)],
    ) -> Result<CmdContext<T>, PluginScriptError> {
        let mut full_env = vec![
            ("PATH", self.script_env_path.as_str()),
            ("QWER_LOG", "trace"),
        ];

        full_env.extend_from_slice(env);
        Ok(crate::prog::run_script(pool, parse_output, script, &full_env)?)
    }

    fn run_script_sync<P: AsRef<Path>>(
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
            .env("PATH", &self.script_env_path)
            .env("QWER_LOG", "trace")
            .stderr_capture()
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

    pub fn list_all(
        &self,
        pool: &ThreadPool,
    ) -> Result<CmdContext<Vec<String>>, PluginScriptError> {
        let list_all_script = self.plugin_dir.join("bin/list-all");
        self.assert_script_exists(&list_all_script)?;
        self.run_script(pool, parse_list_all, &list_all_script, &[])
    }

    pub fn plugin_installed(&self) -> bool {
        self.plugin_dir.is_dir()
    }

    pub fn version_installed(&self, version: &Version) -> bool {
        self.install_dir.join(version.version_str()).is_dir()
    }

    pub fn latest(
        &self,
        pool: &ThreadPool,
    ) -> Result<CmdContext<Option<Version>>, PluginScriptError> {
        let list_all_script = self.plugin_dir.join("bin/list-all");
        self.assert_script_exists(&list_all_script)?;
        self.run_script(pool, parse_find_latest, &list_all_script, &[])
    }

    pub fn has_download(&self) -> bool {
        self.plugin_dir.join("bin/download").is_file()
    }

    pub fn download(
        &self,
        pool: &ThreadPool,
        version: &Version,
    ) -> Option<Result<CmdContext<()>, PluginScriptError>> {
        if version == &Version::System {
            return None;
        }

        let download_script = self.plugin_dir.join("bin/download");
        if !download_script.is_file() {
            return None;
        }

        // TODO: Escape refs and paths correctly
        let version_str = version.version_str();
        let version_download_dir = self.download_dir.join(version_str);
        let version_install_dir = self.install_dir.join(version_str);
        if version_install_dir.is_dir() {
            return Some(Err(PluginScriptError::VersionAlreadyInstalled {
                plugin: self.name.clone(),
                version: version.raw(),
            }));
        }

        match fs::create_dir_all(&version_download_dir) {
            Err(err) => return Some(Err(err.into())),
            _ => {}
        };

        Some(self.run_script(
            pool,
            parse_output_none,
            &download_script,
            &[
                (ASDF_INSTALL_TYPE, version.install_type()),
                (ASDF_INSTALL_VERSION, version_str),
                (ASDF_INSTALL_PATH, &version_install_dir.to_string_lossy()),
                (ASDF_DOWNLOAD_PATH, &version_download_dir.to_string_lossy()),
            ],
        ))
    }

    pub fn install(
        &self,
        pool: &ThreadPool,
        version: &Version,
        concurrency: Option<usize>,
    ) -> Option<Result<CmdContext<String>, PluginScriptError>> {
        trace!(
            "Installing version {version:?} for plugin `{:?}` to `{:?}`",
            self.plugin_dir,
            self.install_dir,
        );

        if version == &Version::System {
            return None;
        }

        let install_script = self.plugin_dir.join("bin/install");
        match self.assert_script_exists(&install_script) {
            Err(err) => return Some(Err(err)),
            _ => {}
        };

        // TODO: Escape refs and paths correctly
        let version_str = version.version_str();
        let version_download_dir = self.download_dir.join(version.version_str());
        let version_install_dir = self.install_dir.join(version.version_str());
        if version_install_dir.is_dir() {
            return Some(Err(PluginScriptError::VersionAlreadyInstalled {
                plugin: self.name.clone(),
                version: version.raw(),
            }));
        }

        match fs::create_dir_all(&version_install_dir) {
            Err(err) => return Some(Err(err.into())),
            _ => {}
        };

        let concurrency = concurrency
            .or_else(|| num_threads::num_threads().map(|num| num.get()))
            .unwrap_or(1);

        Some(self.run_script(
            pool,
            parse_output_passthrough,
            &install_script,
            &[
                (ASDF_INSTALL_TYPE, version.install_type()),
                (ASDF_INSTALL_VERSION, version_str),
                (ASDF_INSTALL_PATH, &version_install_dir.to_string_lossy()),
                (ASDF_DOWNLOAD_PATH, &version_download_dir.to_string_lossy()),
                (ASDF_CONCURRENCY, &concurrency.to_string()),
            ],
        ))
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

    pub fn rm_version_download(&self, version: &Version) -> Result<(), PluginScriptError> {
        let dl_dir = self.download_dir.join(version.version_str());
        if !dl_dir.is_dir() {
            return Ok(());
        }

        Ok(fs::remove_dir_all(&dl_dir)?)
    }

    pub fn uninstall(
        &self,
        pool: &ThreadPool,
        version: &Version,
    ) -> Option<Result<CmdContext<String>, PluginScriptError>> {
        trace!(
            "Uninstalling version {version:?} for plugin `{:?}` from `{:?}`",
            self.plugin_dir,
            self.install_dir,
        );

        if version == &Version::System {
            return None;
        }

        let version_str = version.version_str();
        let version_install_dir = self.install_dir.join(version_str);

        if !version_install_dir.is_dir() {
            return Some(Err(PluginScriptError::VersionNotInstalled {
                plugin: self.name.clone(),
                version: version.raw(),
            }));
        }

        let uninstall_script = self.plugin_dir.join("bin/uninstall");
        match self.assert_script_exists(&uninstall_script) {
            Err(err) => return Some(Err(err)),
            _ => {}
        };

        Some(self.run_script(
            pool,
            parse_output_passthrough,
            &uninstall_script,
            &[
                (ASDF_INSTALL_TYPE, version.install_type()),
                (ASDF_INSTALL_VERSION, version_str),
                (ASDF_INSTALL_PATH, &version_install_dir.to_string_lossy()),
            ],
        ))
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

        let output = self.run_script_sync(&help_path, &env)?;

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
        let output = self.run_script_sync(
            &script_path,
            &[
                (ASDF_INSTALL_TYPE, version.install_type()),
                (ASDF_INSTALL_VERSION, &version.raw()),
                (ASDF_INSTALL_PATH, &*version_dir.to_string_lossy()),
            ],
        )?;

        Ok(output
            .trim()
            .split(' ')
            .map(|path| version_dir.join(path).to_string_lossy().to_string())
            .collect())
    }

    pub fn get_version_path(&self, version: &Version) -> Result<PathBuf, PluginScriptError> {
        let result = self.install_dir.join(version.raw());
        if !result.is_dir() {
            return Err(PluginScriptError::VersionNotInstalled {
                plugin: self.name.clone(),
                version: version.raw(),
            });
        }

        Ok(result)
    }

    // Env modification

    pub fn exec_env_echo(
        &self,
        version: &Version,
    ) -> Result<Option<Vec<(String, String)>>, PluginScriptError> {
        // TODO: Do we need to support adding to path entries here?

        let version_dir = self.install_dir.join(version.version_str());
        let exec_path = self.plugin_dir.join("bin/exec-env");
        if !exec_path.is_file() {
            return Ok(None);
        }

        // This is an alternative version of printing the env, which should
        // be faster since it doesn't need to diff the entire env.
        // By replacing every instance of `export` with `echo`, we make
        // the script output the paths that should be added.
        // This might be less accurate and might not capture everything since
        // the script can source another script that exports vars.
        let exec_echo_path = self.plugin_dir.join("bin/exec-env-echo");
        if !exec_echo_path.is_file() {
            trace!("Generating exec-env-echo script");
            let script_contents = fs::read_to_string(&exec_path)?;
            let echo_script = EXPORT_ECHO_RE.replace_all(&script_contents, "echo ");

            trace!("Wrote exec-env-echo script:\n{echo_script}");
            fs::write(&exec_echo_path, echo_script.as_bytes())?;

            // Make the new script executable
            fs::set_permissions(&exec_echo_path, PermissionsExt::from_mode(0o0755))?;
        }

        let output = self.run_script_sync(
            &exec_echo_path,
            &[
                (ASDF_INSTALL_TYPE, version.install_type()),
                (ASDF_INSTALL_VERSION, &version.raw()),
                (ASDF_INSTALL_PATH, &*version_dir.to_string_lossy()),
            ],
        )?;

        let parts = output
            .split('\n')
            .filter_map(|line| line.split_once('='))
            .map(|(key, val)| (key.to_owned(), val.to_owned()))
            .collect::<Vec<_>>();

        if !parts.is_empty() {
            Ok(Some(parts))
        } else {
            Ok(None)
        }
    }

    pub fn exec_env_diff(
        &self,
        version: &Version,
    ) -> Result<Option<Vec<(String, String)>>, PluginScriptError> {
        // TODO: Do we need to support adding to path entries here?

        // This is pretty stupid, but there's no way for us to know
        // what vars are being changed unless we actually run the script
        // and compare the env before and after.
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
            r#"env;echo;{}={} {}={} {}={} . "{}";echo;env;"#,
            ASDF_INSTALL_TYPE,
            ASDF_INSTALL_VERSION,
            ASDF_INSTALL_PATH,
            version.install_type(),
            version.raw(),
            version_dir.to_string_lossy(),
            exec_path.to_string_lossy(),
        );

        let output = duct::cmd!("bash", "-c", &run_str)
            .env("PATH", &self.script_env_path)
            .read()?;

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

    pub fn latest_stable(
        &self,
        pool: &ThreadPool,
    ) -> Result<CmdContext<Option<Version>>, PluginScriptError> {
        let path = self.plugin_dir.join("bin/latest-stable");
        match path.is_file() {
            true => self.run_script(pool, parse_latest_stable, &path, &[]),
            false => {
                let list_all_script = self.plugin_dir.join("bin/list-all");
                self.assert_script_exists(&list_all_script)?;
                self.run_script(pool, parse_find_latest_stable, &list_all_script, &[])
            }
        }
    }

    // Hooks

    pub fn post_plugin_add(
        &self,
        pool: &ThreadPool,
        install_url: &str,
    ) -> Option<Result<CmdContext<()>, PluginScriptError>> {
        let path = self.plugin_dir.join("bin/post-plugin-add");
        if !path.is_file() {
            return None;
        }

        Some(self.run_script(
            pool,
            parse_output_none,
            &path,
            &[(ASDF_PLUGIN_SOURCE_URL, install_url)],
        ))
    }

    pub fn post_plugin_update(
        &self,
        pool: &ThreadPool,
        prev: &str,
        post: &str,
    ) -> Option<Result<CmdContext<()>, PluginScriptError>> {
        let path = self.plugin_dir.join("bin/post-plugin-add");
        if !path.is_file() {
            return None;
        }

        Some(self.run_script(
            pool,
            parse_output_none,
            &path,
            &[
                (ASDF_PLUGIN_PATH, &*self.plugin_dir.to_string_lossy()),
                (ASDF_PLUGIN_PREV_REF, prev),
                (ASDF_PLUGIN_POST_REF, post),
            ],
        ))
    }

    pub fn pre_plugin_remove(
        &self,
        pool: &ThreadPool,
    ) -> Option<Result<CmdContext<()>, PluginScriptError>> {
        let path = self.plugin_dir.join("bin/post-plugin-add");
        if !path.is_file() {
            return None;
        }

        Some(self.run_script(
            pool,
            parse_output_none,
            &path,
            &[(ASDF_PLUGIN_PATH, &*self.plugin_dir.to_string_lossy())],
        ))
    }

    // Extensions

    pub fn list_extensions(&self) -> Result<HashMap<String, PathBuf>, PluginScriptError> {
        let command_dir = self.plugin_dir.join("lib").join("commands");
        if !command_dir.is_dir() {
            return Ok(HashMap::new());
        }

        let files = fs::read_dir(&command_dir)?;
        let mut result = HashMap::new();
        for file in files {
            let file = file?;
            let filename = file.file_name().to_string_lossy().to_string();
            let filepath = file.path();
            let ext = filepath
                .extension()
                .map(|ext| ext.to_string_lossy().to_string())
                .unwrap_or_default();

            if !filename.starts_with("command") || ext != "bash" {
                continue;
            }

            let ext_name = filename.trim_end_matches(".bash").to_owned();
            result.insert(ext_name, filepath);
        }

        Ok(result)
    }

    pub fn extension<P: AsRef<Path>>(
        &self,
        ext: P,
        args: &[&str],
    ) -> Result<(), PluginScriptError> {
        duct::cmd(ext.as_ref(), args)
            .env("PATH", &self.script_env_path)
            .env("QWER_LOG", "trace")
            .run()?;

        Ok(())
    }

    // Helpers

    pub fn get_env(&self, version: &Version) -> Result<Env, PluginScriptError> {
        let mut env = Env::default();

        // first, see if there's an exec-env
        if let Some(exec_env) = self.exec_env_echo(version)? {
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
        todo!()

        // match version {
        //     "latest" => self.latest(),
        //     "latest-stable" => self.latest_stable(),
        //     _ => self.find_version(version),
        // }
    }
}

fn parse_output_none(_output: String) -> () {
    ()
}

fn parse_output_passthrough(output: String) -> String {
    output
}

fn parse_list_all(output: String) -> Vec<String> {
    output.trim().split(' ').map(|v| v.to_owned()).collect()
}

fn parse_find_latest(output: String) -> Option<Version> {
    output
        .trim()
        .split(' ')
        .last()
        .map(|version| Version::parse(version))
}

fn parse_find_latest_stable(output: String) -> Option<Version> {
    output
        .trim()
        .split(' ')
        .filter(|version| !LATEST_STABLE_RE.is_match(version))
        .last()
        .map(|version| Version::parse(version))
}

fn parse_latest_stable(output: String) -> Option<Version> {
    Some(Version::parse(output.trim()))
}
