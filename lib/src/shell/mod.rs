mod bash;
mod zsh;

pub use bash::Bash;
pub use zsh::Zsh;

pub trait Shell {
    fn hook(&self, cmd: &str, hook_fn: &str) -> String;
    fn set(&self, state: &mut ShellState, var: &str, value: &str);
    fn unset(&self, state: &mut ShellState, var: &str);
}

#[derive(Debug, Default)]
pub struct ShellState(String);

impl ShellState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> String {
        self.0
    }

    pub(crate) fn append(&mut self, val: &str) {
        self.0.push_str(val)
    }
}
