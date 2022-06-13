use log::trace;

use super::Shell;

pub struct Bash;

impl Shell for Bash {
    fn hook(&self, cmd: &str, hook_fn: &str) -> String {
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
    fn hook_bash() {
        assert_eq!(
            Bash.hook("\"./foo\" export bash", "foo_hook"),
            String::from(
                r#"_foo_hook() {
  local previous_exit_status=$?;
  trap -- '' SIGINT;
  eval "$("./foo" export bash)";
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
