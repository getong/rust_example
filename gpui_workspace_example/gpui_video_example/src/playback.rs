use std::{
  collections::VecDeque,
  path::{Path, PathBuf},
  sync::mpsc::Receiver,
  time::Duration,
};

use anyhow::{Context, Result, ensure};
use gpu_video::{
  EncodedInputChunk, OutputFrame, VideoDeviceExt, VideoTexture, WgpuTexturesDecoderH264,
  parameters::{DecoderParameters, InputBytestreamFormatH264},
};

use crate::{
  audio::AudioPlayback,
  demuxer::Mp4Demuxer,
  gpu::GpuContext,
  software::{SoftwareFrame, SoftwarePlayback},
};

pub const SUPPORTED_VIDEO_EXTENSIONS: &[&str] = &["mp4"];
pub const PLAYBACK_SPEEDS: &[f32] = &[1.0, 1.25, 1.5, 2.0, 3.0];

pub enum Frame {
  Gpu(OutputFrame<VideoTexture>),
  Software(SoftwareFrame),
}

enum Backend {
  Gpu(Box<GpuPlayback>),
  Software(SoftwarePlayback),
}

pub struct Playback {
  backend: Backend,
  audio: Option<AudioPlayback>,
  path: PathBuf,
  speed: f32,
  pub duration: Duration,
  pub frame_number: u64,
  pub time: Duration,
}

impl Playback {
  pub fn new(gpu: Option<&GpuContext>, path: &Path) -> Result<Self> {
    ensure!(
      path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
          SUPPORTED_VIDEO_EXTENSIONS
            .iter()
            .any(|supported| extension.eq_ignore_ascii_case(supported))
        }),
      "Unsupported media file: only MP4 videos (.mp4) are supported"
    );
    let (backend, duration) = Self::open_backend(gpu, path, Duration::ZERO)?;
    Ok(Self {
      backend,
      audio: AudioPlayback::new(path)?,
      path: path.to_path_buf(),
      speed: 1.0,
      duration,
      frame_number: 0,
      time: Duration::ZERO,
    })
  }

  fn open_backend(
    gpu: Option<&GpuContext>,
    path: &Path,
    position: Duration,
  ) -> Result<(Backend, Duration)> {
    let (demuxer, duration) = Mp4Demuxer::open_video(path)?;
    let backend = match (gpu, demuxer) {
      (Some(gpu), Some(demuxer)) => match GpuPlayback::new(&gpu.device, &gpu.queue, demuxer) {
        Ok(mut playback) => {
          playback.demuxer.seek(position)?;
          playback.seek_target = Some(position);
          Backend::Gpu(Box::new(playback))
        }
        Err(error) => {
          eprintln!("Hardware decoding unavailable, using FFmpeg: {error:#}");
          Backend::Software(SoftwarePlayback::new(path, position, duration)?)
        }
      },
      _ => Backend::Software(SoftwarePlayback::new(path, position, duration)?),
    };
    Ok((backend, duration))
  }

  pub fn seek(&mut self, gpu: Option<&GpuContext>, position: Duration) -> Result<()> {
    let position = position.min(self.duration);
    let (backend, _) = Self::open_backend(gpu, &self.path, position)?;
    if let Some(audio) = &mut self.audio {
      audio.seek(&self.path, position)?;
    }
    self.backend = backend;
    self.time = position;
    self.frame_number = 0;
    Ok(())
  }

  pub fn next_frame(&mut self, elapsed: Duration) -> Result<Option<Frame>> {
    if let Some(audio) = &self.audio {
      audio.check_error()?;
    }
    let time = self
      .audio
      .as_ref()
      .and_then(AudioPlayback::position)
      .unwrap_or_else(|| {
        self
          .time
          .saturating_add(elapsed.mul_f64(f64::from(self.speed)))
      });
    self.time = self.time.max(time).min(self.duration);
    let mut latest = None;
    for _ in 0 .. 8 {
      let frame = match &mut self.backend {
        Backend::Gpu(playback) => playback.next_frame(self.time)?.map(Frame::Gpu),
        Backend::Software(playback) => playback.next_frame(self.time)?.map(Frame::Software),
      };
      let Some(frame) = frame else {
        break;
      };
      self.frame_number += 1;
      latest = Some(frame);
    }
    Ok(latest)
  }

  pub fn set_paused(&self, paused: bool) {
    if let Some(audio) = &self.audio {
      audio.set_paused(paused);
    }
  }

  pub fn set_volume(&self, volume: f32) {
    if let Some(audio) = &self.audio {
      audio.set_volume(volume);
    }
  }

  pub fn set_speed(&mut self, speed: f32) {
    if !PLAYBACK_SPEEDS.contains(&speed) {
      return;
    }
    self.speed = speed;
    if let Some(audio) = &self.audio {
      audio.set_speed(speed);
    }
  }

  pub fn has_audio(&self) -> bool {
    self.audio.is_some()
  }

  pub fn backend_name(&self) -> &'static str {
    match &self.backend {
      Backend::Gpu(_) => "GPU",
      Backend::Software(_) => "FFmpeg",
    }
  }
}

struct GpuPlayback {
  demuxer: Mp4Demuxer,
  decoder: WgpuTexturesDecoderH264,
  frame_receiver: Receiver<OutputFrame<VideoTexture>>,
  eof: bool,
  decoded_frames: VecDeque<OutputFrame<VideoTexture>>,
  seek_target: Option<Duration>,
  seek_frame: Option<OutputFrame<VideoTexture>>,
}

impl GpuPlayback {
  const MAX_DECODED_FRAMES: usize = 3;

  fn new(device: &wgpu::Device, queue: &wgpu::Queue, demuxer: Mp4Demuxer) -> Result<Self> {
    let (frame_sender, frame_receiver) = std::sync::mpsc::channel();
    let parameters = DecoderParameters {
      bytestream_format: InputBytestreamFormatH264::Avcc {
        prefix_length: demuxer.prefix_length,
      },
      ..Default::default()
    };
    let decoder = device
      .video()
      .context("The GPU device does not support video decoding")?
      .create_wgpu_textures_decoder_h264(queue, parameters, move |frame| {
        let _ = frame_sender.send(frame);
      })
      .context("Failed to create the H.264 decoder")?;

    Ok(Self {
      demuxer,
      decoder,
      frame_receiver,
      eof: false,
      decoded_frames: VecDeque::new(),
      seek_target: None,
      seek_frame: None,
    })
  }

  fn next_frame(&mut self, time: Duration) -> Result<Option<OutputFrame<VideoTexture>>> {
    if let Some(target) = self.seek_target {
      for _ in 0 .. 64 {
        let Some(frame) = self.decode_next()? else {
          if self.eof {
            self.seek_target = None;
            return Ok(self.seek_frame.take());
          }
          return Ok(None);
        };
        if Duration::from_micros(
          frame
            .metadata
            .pts
            .context("Missing decoded frame timestamp")?,
        ) >= target
        {
          self.seek_target = None;
          self.seek_frame = None;
          return Ok(Some(frame));
        }
        self.seek_frame = Some(frame);
      }
      return Ok(None);
    }
    while self.decoded_frames.len() < Self::MAX_DECODED_FRAMES {
      let Some(frame) = self.decode_next()? else {
        break;
      };
      self.decoded_frames.push_back(frame);
    }

    let is_due = self
      .decoded_frames
      .front()
      .is_some_and(|f| Duration::from_micros(f.metadata.pts.unwrap()) <= time);
    if !is_due {
      return Ok(None);
    }
    Ok(self.decoded_frames.pop_front())
  }

  fn decode_next(&mut self) -> Result<Option<OutputFrame<VideoTexture>>> {
    loop {
      if let Ok(frame) = self.frame_receiver.try_recv() {
        return Ok(Some(frame));
      }

      if self.eof {
        return Ok(None);
      }

      match self.demuxer.next_chunk()? {
        Some(chunk) => self
          .decoder
          .decode(EncodedInputChunk {
            data: &chunk.data,
            pts: Some(chunk.pts.as_micros() as u64),
          })
          .context("Failed to decode an H.264 frame")?,
        None => {
          self
            .decoder
            .flush()
            .context("Failed to flush the H.264 decoder")?;
          self.eof = true;
        }
      }
    }
  }
}
