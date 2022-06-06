use anyhow::{Result, bail};

pub fn run_script(command: &str, env: &[(&str, &str)]) -> Result<String> {
    let mut expr = duct::cmd!(command)
        .stderr_to_stdout()
        .stdout_capture()
        .unchecked();

    for (key, val) in env {
        expr = expr.env(key, val);
    }

    let output = expr.run()?;

    let output_str = String::from_utf8(output.stdout)?;
    if !output.status.success() {
        bail!("{output_str}");
    }

    Ok(output_str)
}
