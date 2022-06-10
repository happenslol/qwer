use std::collections::HashMap;

pub mod plugins;
pub mod scripts;
pub mod shell;
pub mod versions;

#[derive(Debug, Default)]
pub struct Env {
    pub path: Vec<String>,
    pub vars: HashMap<String, String>,
    pub run: Vec<String>,
}

impl Env {
    pub fn merge(&mut self, other: Env) {
        self.path.extend(other.path);
        self.run.extend(other.run);

        for (key, val) in other.vars {
            self.vars.insert(key, val);
        }
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
            run: vec!["echo foo".to_owned()],
        }
    }
}
