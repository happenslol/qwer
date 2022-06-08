use log::trace;

use crate::{Env, Shell};

pub struct Bash;

impl Shell for Bash {
    fn hook(cmd: &str, hook_fn: &str) -> String {
        let result = format!(
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
        );

        trace!("inserting hook function into bash:\n{result}");

        result
    }

    fn export(env: &Env) -> String {
        trace!("exporting bash env:\n{env:#?}");

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

        format!("export PATH=$PATH:{path};{vars}{runs}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_env;

    #[test]
    fn export_bash() {
        let env = test_env();
        let result = Bash::export(&env);

        assert!(result.contains("export PATH=$PATH:foo:bar;"));
        assert!(result.contains("export foo=bar;"));
        assert!(result.contains("export baz=foo;"));
        assert!(result.contains("echo foo;"));
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
}
