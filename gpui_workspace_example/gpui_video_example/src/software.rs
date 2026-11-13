use std::{
  io::{BufRead, BufReader, ErrorKind, Read},
  path::Path,
  process::{Child, Command, Stdio},
  sync::mpsc::{self, Receiver, TryRecvError},
  thread,
  time::Duration,
};

use anyhow::{Context, Result, ensure};

use crate::media::MediaInfo;

pub struct SoftwareFrame {
  pub pixels: Vec<u8>,
  pub width: u32,
  pub height: u32,
  pub pts: Duration,
}

pub struct SoftwarePlayback {
  child: Child,
  frames: Receiver<Result<SoftwareFrame>>,
  pending: Option<SoftwareFrame>,
  finished: bool,
}

impl SoftwarePlayback {
  pub fn new(path: &Path, position: Duration, info: &MediaInfo) -> Result<Self> {
    let (width, height) = info.display_size();
    let frame_size = (width as usize)
      .checked_mul(height as usize)
      .and_then(|size| size.checked_mul(4))
      .filter(|size| *size > 0 && *size <= 256 * 1024 * 1024)
      .context("invalid video dimensions")?;
    let start = position.min(
      info
        .duration
        .saturating_sub(Duration::from_secs_f64(1.0 / info.fps)),
    );
    let mut child = Command::new("ffmpeg")
      .args(["-nostdin", "-v", "info", "-copyts", "-ss"])
      .arg(format!("{:.9}", start.as_secs_f64()))
      .arg("-i")
      .arg(path)
      .args([
        "-map",
        &format!("0:{}", info.video_index),
        "-an",
        "-sn",
        "-dn",
        "-vf",
        "showinfo",
        "-fps_mode",
        "passthrough",
        "-pix_fmt",
        "bgra",
        "-f",
        "rawvideo",
        "pipe:1",
      ])
      .stdin(Stdio::null())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .context("could not start FFmpeg video decoding")?;
    let mut stdout = child.stdout.take().context("missing video pipe")?;
    let stderr = child.stderr.take().context("missing video diagnostics")?;
    let (timestamp_sender, timestamps) = mpsc::channel();
    thread::spawn(move || {
      for line in BufReader::new(stderr).lines().map_while(Result::ok) {
        if line.contains("showinfo")
          && let Some((_, tail)) = line.split_once("pts_time:")
          && let Some(seconds) = tail
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<f64>().ok())
          && let Ok(pts) = Duration::try_from_secs_f64(seconds.max(0.0))
          && timestamp_sender.send(pts).is_err()
        {
          break;
        }
      }
    });
    let (sender, frames) = mpsc::sync_channel(3);
    thread::spawn(move || {
      loop {
        let mut pixels = vec![0; frame_size];
        match stdout.read_exact(&mut pixels[.. 1]) {
          Err(error) if error.kind() == ErrorKind::UnexpectedEof => break,
          Err(error) => {
            let _ = sender.send(Err(error.into()));
            break;
          }
          Ok(()) => {}
        }
        if let Err(error) = stdout.read_exact(&mut pixels[1 ..]) {
          let _ = sender.send(Err(error).context("incomplete video frame"));
          break;
        }
        let frame = timestamps
          .recv_timeout(Duration::from_secs(5))
          .context("missing decoded frame timestamp")
          .map(|pts| SoftwareFrame {
            pixels,
            width,
            height,
            pts,
          });
        let failed = frame.is_err();
        if sender.send(frame).is_err() || failed {
          break;
        }
      }
    });
    Ok(Self {
      child,
      frames,
      pending: None,
      finished: false,
    })
  }

  pub fn next_frame(&mut self, time: Duration) -> Result<Option<SoftwareFrame>> {
    if self.finished {
      return Ok(None);
    }
    if self.pending.is_none() {
      match self.frames.try_recv() {
        Ok(frame) => self.pending = Some(frame?),
        Err(TryRecvError::Empty) => return Ok(None),
        Err(TryRecvError::Disconnected) => {
          self.finished = true;
          let status = self.child.wait()?;
          ensure!(status.success(), "FFmpeg video decoder failed ({status})");
          return Ok(None);
        }
      }
    }
    if self.pending.as_ref().is_some_and(|frame| frame.pts <= time) {
      Ok(self.pending.take())
    } else {
      Ok(None)
    }
  }
}

impl Drop for SoftwarePlayback {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}
