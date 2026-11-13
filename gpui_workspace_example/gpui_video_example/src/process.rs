use std::{
  io::Read,
  process::{Command, Output, Stdio},
  thread,
  time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};

pub fn output(command: &mut Command, timeout: Duration) -> Result<Output> {
  let mut child = command
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .stdin(Stdio::null())
    .spawn()
    .context("could not start FFmpeg/ffprobe; install ffmpeg and add it to PATH")?;
  let mut stdout = child.stdout.take().context("missing process stdout")?;
  let mut stderr = child.stderr.take().context("missing process stderr")?;
  let stdout = thread::spawn(move || {
    let mut bytes = Vec::new();
    stdout.read_to_end(&mut bytes).map(|_| bytes)
  });
  let stderr = thread::spawn(move || {
    let mut bytes = Vec::new();
    stderr.read_to_end(&mut bytes).map(|_| bytes)
  });
  let started = Instant::now();
  let status = loop {
    if let Some(status) = child.try_wait()? {
      break status;
    }
    if started.elapsed() > timeout {
      let _ = child.kill();
      let _ = child.wait();
      bail!(
        "media process timed out after {} seconds",
        timeout.as_secs()
      );
    }
    thread::sleep(Duration::from_millis(10));
  };
  Ok(Output {
    status,
    stdout: stdout
      .join()
      .map_err(|_| anyhow::anyhow!("stdout reader failed"))??,
    stderr: stderr
      .join()
      .map_err(|_| anyhow::anyhow!("stderr reader failed"))??,
  })
}
