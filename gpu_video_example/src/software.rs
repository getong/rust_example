use std::{
  io::{ErrorKind, Read},
  path::Path,
  process::{Child, Command, Stdio},
  sync::mpsc::{self, Receiver, TryRecvError},
  thread::{self, JoinHandle},
  time::Duration,
};

use anyhow::{Context, Result, bail, ensure};

pub struct SoftwareFrame {
  pub pixels: Vec<u8>,
  pub width: u32,
  pub height: u32,
}

pub struct SoftwarePlayback {
  child: Child,
  frames: Receiver<Result<SoftwareFrame>>,
  stderr: Option<JoinHandle<String>>,
  frame_rate: f64,
  next_frame: u64,
  finished: bool,
}

impl SoftwarePlayback {
  pub fn new(path: &Path) -> Result<Self> {
    let probe = Command::new("ffprobe")
      .args([
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=width,height,avg_frame_rate",
        "-of",
        "default=noprint_wrappers=1",
      ])
      .arg(path)
      .output()
      .context(
        "This codec needs FFmpeg software decoding; install ffmpeg and ffprobe and put them on \
         PATH",
      )?;
    ensure!(
      probe.status.success(),
      "Failed to inspect video: {}",
      String::from_utf8_lossy(&probe.stderr)
    );
    let metadata = String::from_utf8(probe.stdout).context("Invalid ffprobe output")?;
    let field = |name: &str| {
      metadata
        .lines()
        .find_map(|line| line.strip_prefix(name))
        .with_context(|| format!("Missing video metadata: {name}"))
    };
    let width: u32 = field("width=")?.parse().context("Invalid video width")?;
    let height: u32 = field("height=")?.parse().context("Invalid video height")?;
    let rate = field("avg_frame_rate=")?;
    let (numerator, denominator) = rate.split_once('/').context("Invalid video frame rate")?;
    let frame_rate = numerator.parse::<f64>()? / denominator.parse::<f64>()?;
    ensure!(
      frame_rate.is_finite() && frame_rate > 0.0,
      "Invalid video frame rate: {rate}"
    );
    let frame_size = (width as usize)
      .checked_mul(height as usize)
      .and_then(|size| size.checked_mul(4))
      .filter(|size| *size > 0 && *size <= 256 * 1024 * 1024)
      .context("Invalid or excessively large video dimensions")?;

    let mut child = Command::new("ffmpeg")
      .args(["-nostdin", "-v", "error", "-noautorotate", "-i"])
      .arg(path)
      .args([
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-vf",
        &format!("fps={rate}"),
        "-pix_fmt",
        "rgba",
        "-f",
        "rawvideo",
        "pipe:1",
      ])
      .stdin(Stdio::null())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .context("Failed to start FFmpeg; install ffmpeg and put it on PATH")?;
    let mut stdout = child.stdout.take().context("Missing FFmpeg video pipe")?;
    let mut stderr = child.stderr.take().context("Missing FFmpeg error pipe")?;
    let stderr = thread::spawn(move || {
      let mut tail = Vec::new();
      let mut buffer = [0; 4096];
      while let Ok(count) = stderr.read(&mut buffer) {
        if count == 0 {
          break;
        }
        tail.extend_from_slice(&buffer[.. count]);
        if tail.len() > 16384 {
          tail.drain(.. tail.len() - 16384);
        }
      }
      String::from_utf8_lossy(&tail).into_owned()
    });
    let (sender, frames) = mpsc::sync_channel(3);
    thread::spawn(move || {
      loop {
        let mut pixels = vec![0; frame_size];
        match stdout.read_exact(&mut pixels[.. 1]) {
          Err(error) if error.kind() == ErrorKind::UnexpectedEof => break,
          Err(error) => {
            let _ = sender.send(Err(error).context("Failed to read FFmpeg video output"));
            break;
          }
          Ok(()) => {}
        }
        if let Err(error) = stdout.read_exact(&mut pixels[1 ..]) {
          let _ = sender.send(Err(error).context("Incomplete FFmpeg video frame"));
          break;
        }
        if sender
          .send(Ok(SoftwareFrame {
            pixels,
            width,
            height,
          }))
          .is_err()
        {
          break;
        }
      }
    });

    Ok(Self {
      child,
      frames,
      stderr: Some(stderr),
      frame_rate,
      next_frame: 0,
      finished: false,
    })
  }

  pub fn next_frame(&mut self, time: Duration) -> Result<Option<SoftwareFrame>> {
    if self.finished || self.next_frame as f64 / self.frame_rate > time.as_secs_f64() {
      return Ok(None);
    }
    match self.frames.try_recv() {
      Ok(frame) => {
        self.next_frame += 1;
        frame.map(Some)
      }
      Err(TryRecvError::Empty) => Ok(None),
      Err(TryRecvError::Disconnected) => {
        self.finished = true;
        let status = self.child.wait().context("Failed to wait for FFmpeg")?;
        let stderr = self
          .stderr
          .take()
          .and_then(|thread| thread.join().ok())
          .unwrap_or_default();
        if !status.success() {
          bail!("FFmpeg video decoding failed ({status}): {stderr}");
        }
        ensure!(self.next_frame > 0, "FFmpeg produced no video frames");
        Ok(None)
      }
    }
  }
}

impl Drop for SoftwarePlayback {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}
