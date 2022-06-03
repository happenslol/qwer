use crate::{Shell, Env};

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

