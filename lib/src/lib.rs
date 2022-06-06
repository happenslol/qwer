use std::collections::HashMap;

pub mod plugins;
pub mod scripts;
pub mod shell;
pub mod versions;

pub struct Env<'a> {
    pub path: Vec<&'a str>,
    pub vars: HashMap<&'a str, &'a str>,
}

pub trait Shell {
    fn hook(cmd: &str, hook_fn: &str) -> String;
    fn export(env: &Env) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn test_env<'a>() -> Env<'a> {
        Env {
            path: vec!["foo", "bar"],
            vars: HashMap::from([("foo", "bar"), ("baz", "foo")]),
        }
    }
}
