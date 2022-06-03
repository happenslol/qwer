use std::collections::HashMap;

pub struct Env<'a> {
    pub path: Vec<&'a str>,
    pub vars: HashMap<&'a str, &'a str>,
}

pub trait Shell {
    fn hook(cmd: &str, hook_fn: &str) -> String;
    fn export(env: &Env) -> String;
}

pub struct Bash;

impl Shell for Bash {
    fn hook(cmd: &str, hook_fn: &str) -> String {
        format!(
            r#"_{hook_fn}() {{
  local previous_exit_status=$?;
  trap -- '' SIGINT;
  eval "$({cmd})";
  trap - SIGINT;
  return $previous_exit_status;
}};
if ! [[ "${{PROMPT_COMMAND:-}}" =~ _{hook_fn} ]]; then
  PROMPT_COMMAND="_{hook_fn}${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}"
fi"#
        )
    }

    fn export(env: &Env) -> String {
        let path = env.path.join(":");
        let vars = env
            .vars
            .iter()
            .map(|(key, val)| format!("export {key}={val};"))
            .collect::<Vec<String>>()
            .join("");

        format!("export PATH=$PATH:{path};{vars}")
    }
}

pub struct Zsh;

impl Shell for Zsh {
    fn hook(cmd: &str, hook_fn: &str) -> String {
        format!(
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
        )
    }

    fn export(env: &Env) -> String {
        let path = env.path.join(":");
        let vars = env
            .vars
            .iter()
            .map(|(key, val)| format!("export {key}={val};"))
            .collect::<Vec<String>>()
            .join("");

        format!("export PATH=$PATH:{path};{vars}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_env<'a>() -> Env<'a> {
        Env {
            path: vec!["foo", "bar"],
            vars: HashMap::from([("foo", "bar"), ("baz", "foo")]),
        }
    }

    #[test]
    fn export_bash() {
        let env = test_env();
        let result = Bash::export(&env);

        assert!(result.contains("export PATH=$PATH:foo:bar;"));
        assert!(result.contains("export foo=bar;"));
        assert!(result.contains("export baz=foo;"));
    }

    #[test]
    fn hook_bash() {
        assert_eq!(
            Bash::hook("\"./foo\" hook bash", "foo_hook"),
            String::from(
                r#"_foo_hook() {
  local previous_exit_status=$?;
  trap -- '' SIGINT;
  eval "$("./foo" hook bash)";
  trap - SIGINT;
  return $previous_exit_status;
};
if ! [[ "${PROMPT_COMMAND:-}" =~ _foo_hook ]]; then
  PROMPT_COMMAND="_foo_hook${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi"#
            )
        );
    }

    #[test]
    fn export_zsh() {
        let env = test_env();
        let result = Zsh::export(&env);
        assert!(result.contains("export PATH=$PATH:foo:bar;"));
        assert!(result.contains("export foo=bar;"));
        assert!(result.contains("export baz=foo;"));
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
