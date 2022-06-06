use std::{path::Path, fs};

use anyhow::{Result, bail};

pub fn run_script<P: AsRef<Path>>(path: P) -> Result<String> {
    let script_contents = fs::read_to_string(path)?;
    let output = duct::cmd("bash", &["-c", &script_contents])
        .stderr_to_stdout()
        .stdout_capture()
        .unchecked()
        .run()?;

    let output_str = String::from_utf8(output.stdout)?;
    if !output.status.success() {
        bail!("{output_str}");
    }

    Ok(output_str)
}
