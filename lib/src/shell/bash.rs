use log::trace;

use crate::Shell;

pub struct Bash;

impl Shell for Bash {
    fn hook(cmd: &str, hook_fn: &str) -> String {
        let result = format!(
            r#"_{hook_fn}() {{
  local previous_exit_status=$?;
  trap -- '' SIGINT;
  {cmd};
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_bash() {
        assert_eq!(
            Bash::hook("\"./foo\" export bash", "foo_hook"),
            String::from(
                r#"_foo_hook() {
  local previous_exit_status=$?;
  trap -- '' SIGINT;
  "./foo" export bash;
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
