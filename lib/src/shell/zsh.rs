use log::trace;

use crate::{Env, Shell};

pub struct Zsh;

impl Shell for Zsh {
    fn hook(cmd: &str, hook_fn: &str) -> String {
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

    fn export(env: &Env) -> String {
        trace!("exporting zsh env:\n{env:#?}");

        let path = env.path.join(":");
        let vars = env
            .vars
            .iter()
            .map(|(key, val)| format!("export {key}={val};"))
            .collect::<Vec<String>>()
            .join("");

        let runs = env
            .run
            .iter()
            .map(|cmd| format!("{cmd};"))
            .collect::<Vec<String>>()
            .join("");

        if !path.is_empty() {
            format!("export PATH=$PATH:{path};{vars}{runs}")
        } else {
            format!("{vars}{runs}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_env;

    #[test]
    fn export_zsh() {
        let env = test_env();
        let result = Zsh::export(&env);
        assert!(result.contains("export PATH=$PATH:foo:bar;"));
        assert!(result.contains("export foo=bar;"));
        assert!(result.contains("export baz=foo;"));
        assert!(result.contains("echo foo;"));
    }

    #[test]
    fn hook_zsh() {
        assert_eq!(
            Zsh::hook("\"./foo\" hook zsh", "foo_hook"),
            String::from(
                r#"_foo_hook() {
  trap -- '' SIGINT;
  eval "$("./foo" hook zsh)";
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
