use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    hash::Hasher,
    io::{Read, Write},
};

use lazy_static::lazy_static;
use thiserror::Error;

lazy_static! {
    // See https://github.com/direnv/direnv/blob/master/internal/cmd/env_diff.go
    pub static ref IGNORED_ENV_VARS: HashSet<&'static str> = HashSet::from_iter([
        // Ignore our own vars
        "QWER_STATE",
        "QWER_PREV",
        "QWER_CURRENT",

        // Ignore asdf vars
        "ASDF_INSTALL_TYPE",
        "ASDF_INSTALL_VERSION",
        "ASDF_INSTALL_PATH",
        "ASDF_DOWNLOAD_PATH",
        "ASDF_CONCURRENCY",
        "ASDF_PLUGIN_PATH",
        "ASDF_PLUGIN_PREV_REF",
        "ASDF_PLUGIN_POST_REF",
        "ASDF_PLUGIN_SOURCE_URL",

        // We set the path separately
        "PATH",

        // Bash fixes
        "COMP_WORDBREAKS",
        "PS1",

        // Variables that can change without impacting env
        "SHLVL",
        "SHELLOPTS",
        "SHELL",
        "PWD",
        "OLDPWD",
        "_",
    ]);
}

#[derive(Debug, Default)]
pub struct Env {
    pub path: BTreeSet<String>,
    pub vars: BTreeMap<String, String>,
}

#[derive(Debug, Error)]
pub enum EnvError {
    #[error("env strings must contain 2 segments")]
    InvalidEnvString,

    #[error("string was not valid utf-8: {0}")]
    InvalidString(#[from] std::string::FromUtf8Error),

    #[error("failed to decode env string from base64: {0}")]
    DecodeFailed(#[from] base64::DecodeError),

    #[error("io error while reading or writing env: {0}")]
    Io(#[from] std::io::Error),
}

impl Env {
    pub fn merge(&mut self, other: Env) {
        self.path.extend(other.path);

        for (key, val) in other.vars {
            self.vars.insert(key, val);
        }
    }

    pub fn serialize(&self) -> String {
        let vars_str = self
            .vars
            .iter()
            // It's fine if = appears in the var, since we'll split_once
            // later instead of splitting at every =
            .map(|(key, val)| format!("{}={}", key, base64::encode(val)))
            .collect::<Vec<_>>()
            .join("\n");

        let vars_writer = base64::write::EncoderStringWriter::new(base64::STANDARD);
        let mut vars_writer = snap::write::FrameEncoder::new(vars_writer);
        vars_writer
            .write(vars_str.as_bytes())
            .expect("Failed to write vars");

        let path_str = self.path.iter().cloned().collect::<Vec<_>>().join("\n");
        let path_writer = base64::write::EncoderStringWriter::new(base64::STANDARD);
        let mut path_writer = snap::write::FrameEncoder::new(path_writer);
        path_writer
            .write(path_str.as_bytes())
            .expect("Failed to write path");

        [
            vars_writer
                .into_inner()
                .expect("Failed to flush vars")
                .into_inner(),
            path_writer
                .into_inner()
                .expect("Failed to flush path")
                .into_inner(),
        ]
        .map(|part| base64::encode(part))
        .join(".")
    }

    pub fn deserialize(from: &str) -> Result<Self, EnvError> {
        let parts = from.split('.').collect::<Vec<_>>();
        if parts.len() != 2 {
            return Err(EnvError::InvalidEnvString);
        }

        let mut vars_reader = StringReader::new(parts[0]);
        let vars_reader = base64::read::DecoderReader::new(&mut vars_reader, base64::STANDARD);
        let mut vars_reader = snap::read::FrameDecoder::new(vars_reader);
        let mut vars_str = String::new();
        vars_reader.read_to_string(&mut vars_str)?;
        let mut vars = BTreeMap::new();

        for entry in vars_str.split('\n') {
            let (key, val) = entry.split_once('=').ok_or(EnvError::InvalidEnvString)?;
            let decoded_val = String::from_utf8(base64::decode(val)?)?;
            vars.insert(key.to_owned(), decoded_val);
        }

        let mut path_reader = StringReader::new(parts[1]);
        let path_reader = base64::read::DecoderReader::new(&mut path_reader, base64::STANDARD);
        let mut path_reader = snap::read::FrameDecoder::new(path_reader);
        let mut path_str = String::new();
        path_reader.read_to_string(&mut path_str)?;

        let path = path_str
            .split('\n')
            .map(|entry| entry.to_owned())
            .collect::<BTreeSet<String>>();

        Ok(Self { vars, path })
    }

    pub fn hash(&self) -> u64 {
        let mut hasher = twox_hash::XxHash64::with_seed(0);
        for (key, val) in &self.vars {
            hasher.write(key.as_bytes());
            hasher.write(val.as_bytes());
        }

        for entry in &self.path {
            hasher.write(entry.as_bytes());
        }

        hasher.finish()
    }
}

struct StringReader<'a> {
    iter: std::slice::Iter<'a, u8>,
}

impl<'a> StringReader<'a> {
    pub fn new(data: &'a str) -> Self {
        Self {
            iter: data.as_bytes().iter(),
        }
    }
}

impl<'a> std::io::Read for StringReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        for i in 0..buf.len() {
            if let Some(x) = self.iter.next() {
                buf[i] = *x;
            } else {
                return Ok(i);
            }
        }
        Ok(buf.len())
    }
}
