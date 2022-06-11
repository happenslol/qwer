use std::collections::HashMap;

use thiserror::Error;

pub mod plugins;
pub mod scripts;
pub mod shell;
pub mod versions;

#[derive(Debug, Default)]
pub struct Env {
    pub path: Vec<String>,
    pub vars: HashMap<String, String>,
}

#[derive(Debug, Error)]
pub enum EnvError {
    #[error("env strings must contain 3 segments")]
    InvalidEnvString,

    #[error("string was not valid utf-8: {0}")]
    InvalidString(#[from] std::string::FromUtf8Error),

    #[error("failed to decode env string from base64: {0}")]
    DecodeFailed(#[from] base64::DecodeError),
}

impl Env {
    pub fn merge(&mut self, other: Env) {
        self.path.extend(other.path);

        for (key, val) in other.vars {
            self.vars.insert(key, val);
        }
    }

    pub fn serialize(&self) -> String {
        let mut sorted_vars = self.vars.iter().collect::<Vec<_>>();
        sorted_vars.sort_by_key(|var| var.0);
        let vars_str = sorted_vars
            .iter()
            // It's fine if = appears in the var, since we'll split_once
            // later instead of splitting at every =
            .map(|(key, val)| format!("{}={}", key, base64::encode(val)))
            .collect::<Vec<_>>()
            .join("\n");

        let mut sorted_path = self.path.clone();
        sorted_path.sort();
        let path_str = sorted_path.join("\n");

        [vars_str, path_str]
            .map(|part| base64::encode(part))
            .join(".")
    }

    pub fn deserialize(from: &str) -> Result<Self, EnvError> {
        let parts = from.split('.').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err(EnvError::InvalidEnvString);
        }

        let mut result = Self::default();

        let vars_str = String::from_utf8(base64::decode(parts[0])?)?;
        for entry in vars_str.split('\n') {
            let (key, val) = entry.split_once('=').ok_or(EnvError::InvalidEnvString)?;
            let decoded_val = String::from_utf8(base64::decode(val)?)?;
            result.vars.insert(key.to_owned(), decoded_val);
        }

        result.path = String::from_utf8(base64::decode(parts[1])?)?
            .split('\n')
            .map(|entry| entry.to_owned())
            .collect::<Vec<String>>();

        Ok(result)
    }
}

pub trait Shell {
    fn hook(cmd: &str, hook_fn: &str) -> String;
    fn export(env: &Env) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn test_env() -> Env {
        Env {
            path: vec!["foo".to_owned(), "bar".to_owned()],
            vars: HashMap::from([
                ("foo".to_owned(), "bar".to_owned()),
                ("baz".to_owned(), "foo".to_owned()),
            ]),
        }
    }
}
