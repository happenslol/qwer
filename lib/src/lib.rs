pub mod env;
pub mod plugins;
pub mod scripts;
pub mod shell;
pub mod versions;

pub trait Shell {
    fn hook(cmd: &str, hook_fn: &str) -> String;
}
