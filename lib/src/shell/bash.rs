use crate::{Shell, Env};

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
