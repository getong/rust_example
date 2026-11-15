use anyhow::Result;

#[cfg(unix)]
pub fn detach_from_terminal() -> Result<bool> {
  use std::{
    fs::{self, OpenOptions},
    io::IsTerminal as _,
    os::unix::process::CommandExt as _,
    process::{Command, Stdio},
  };

  use anyhow::Context as _;

  if !std::io::stdin().is_terminal()
    || std::env::var_os("GPUI_VIDEO_FOREGROUND").is_some_and(|value| value == "1")
  {
    return Ok(false);
  }

  let program = std::env::current_exe().context("could not locate the player executable")?;
  let log_path = crate::settings::Settings::path().with_extension("log");
  if let Some(parent) = log_path
    .parent()
    .filter(|parent| !parent.as_os_str().is_empty())
  {
    fs::create_dir_all(parent).context("could not create the player log directory")?;
  }
  let log = OpenOptions::new()
    .create(true)
    .append(true)
    .open(&log_path)
    .with_context(|| format!("could not open player log {}", log_path.display()))?;
  let mut command = Command::new(program);
  command
    .args(std::env::args_os().skip(1))
    .stdin(Stdio::null())
    .stdout(log.try_clone().context("could not clone the player log")?)
    .stderr(log);
  unsafe {
    command.pre_exec(|| {
      if libc::setsid() == -1 {
        return Err(std::io::Error::last_os_error());
      }
      Ok(())
    });
  }
  command
    .spawn()
    .context("could not start the player in the background")?;
  eprintln!("Player started in background; log: {}", log_path.display());
  Ok(true)
}

#[cfg(not(unix))]
pub fn detach_from_terminal() -> Result<bool> {
  Ok(false)
}
