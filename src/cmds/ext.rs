use std::fs::{self, DirEntry};

use anyhow::{bail, Result};
use log::trace;

use crate::{
  cmds::util::print_help_and_exit,
  dirs::{get_dir, get_plugin_scripts, PLUGINS_DIR},
};

pub fn ext(args: Vec<String>) -> Result<()> {
  // Args will always have at least length 1, since we're
  // receiving a subcommand from clap.

  let plugins_dir = get_dir(PLUGINS_DIR)?;
  let installed_plugins = fs::read_dir(&plugins_dir)?.collect::<Result<Vec<DirEntry>, _>>()?;

  let found = installed_plugins
    .iter()
    .find(|dir| dir.path().is_dir() && dir.file_name().to_string_lossy() == args[0]);

  if found.is_none() {
    print_help_and_exit();
  }

  let name = args[0].clone();
  let scripts = get_plugin_scripts(&name)?;
  let cmds = scripts.list_extensions()?;

  let cmd_args = &["command"]
    .into_iter()
    .chain(args.iter().skip(1).map(|it| it.as_str()))
    .collect::<Vec<_>>();

  // build up a list of permutations
  let mut permutations = vec![];
  for i in 1..cmd_args.len() {
    let perm = cmd_args[0..=i].join("-");
    permutations.push(perm);
  }

  // try to match the longest command name first
  permutations.reverse();
  let found_cmd = permutations.iter().enumerate().find_map(|(i, cmd)| {
    trace!("Checking command permutation {cmd}");
    cmds
      .get_key_value(cmd)
      .map(|(cmd, path)| (cmd_args.len() - i, cmd, path))
  });

  if found_cmd.is_none() {
    let cmd_parts = args.into_iter().skip(1).collect::<Vec<_>>().join(" ");
    bail!("Command `{cmd_parts}` not found for plugin `{name}`");
  }

  let (i, cmd, cmd_path) = found_cmd.unwrap();
  let rest_args = &cmd_args[i..cmd_args.len()];

  trace!("Running command: {cmd:?} {cmd_path:?} rest args: {rest_args:?}");
  scripts.extension(&cmd_path, rest_args)?;

  Ok(())
}
