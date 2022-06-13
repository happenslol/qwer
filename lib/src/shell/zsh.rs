use log::trace;

use super::Shell;

pub struct Zsh;

impl Shell for Zsh {
    fn hook(&self, cmd: &str, hook_fn: &str) -> String {
        let result = format!(
            r#"_{hook_fn}() {{
  trap -- '' SIGINT;
  eval "$({cmd})";
  trap - SIGINT;
}}
typeset -ag precmd_functions;
if [[ -z "${{precmd_functions[(r)_{hook_fn}]+1}}" ]]; then
  precmd_functions=( _{hook_fn} ${{precmd_functions[@]}} )
fi
typeset -ag chpwd_functions;
if [[ -z "${{chpwd_functions[(r)_{hook_fn}]+1}}" ]]; then
  chpwd_functions=( _{hook_fn} ${{chpwd_functions[@]}} )
fi"#
        );

        trace!("inserting hook function into zsh:\n{result}");

        result
    }

    fn set(&self, state: &mut super::ShellState, var: &str, value: &str) {
        state.append(&format!("export {var}={value};"));
    }

    fn unset(&self, state: &mut super::ShellState, var: &str) {
        state.append(&format!("unset {var};"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_zsh() {
        assert_eq!(
            Zsh.hook("\"./foo\" export zsh", "foo_hook"),
            String::from(
                r#"_foo_hook() {
  trap -- '' SIGINT;
  eval "$("./foo" export zsh)";
  trap - SIGINT;
}
typeset -ag precmd_functions;
if [[ -z "${precmd_functions[(r)_foo_hook]+1}" ]]; then
  precmd_functions=( _foo_hook ${precmd_functions[@]} )
fi
typeset -ag chpwd_functions;
if [[ -z "${chpwd_functions[(r)_foo_hook]+1}" ]]; then
  chpwd_functions=( _foo_hook ${chpwd_functions[@]} )
fi"#
            )
        );
    }
}
