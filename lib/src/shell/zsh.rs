use log::trace;

use crate::Shell;

pub struct Zsh;

impl Shell for Zsh {
    fn hook(cmd: &str, hook_fn: &str) -> String {
        let result = format!(
            r#"_{hook_fn}() {{
  trap -- '' SIGINT;
  {cmd};
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_zsh() {
        assert_eq!(
            Zsh::hook("\"./foo\" export zsh", "foo_hook"),
            String::from(
                r#"_foo_hook() {
  trap -- '' SIGINT;
  "./foo" export zsh;
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
