use std::{collections::VecDeque, path::Path, sync::mpsc::Receiver, time::Duration};

use anyhow::{Context, Result};
use egui_wgpu::wgpu;
use gpu_video::{
  EncodedInputChunk, OutputFrame, VideoDeviceExt, VideoTexture, WgpuTexturesDecoderH264,
  parameters::{DecoderParameters, InputBytestreamFormatH264},
};

use crate::{
  demuxer::Mp4Demuxer,
  software::{SoftwareFrame, SoftwarePlayback},
};

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
  pub frame_number: u64,
  pub time: Duration,
}

impl Playback {
  pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, path: &Path) -> Result<Self> {
    let backend = match Mp4Demuxer::open_h264(path)? {
      Some(demuxer) => Backend::Gpu(Box::new(GpuPlayback::new(device, queue, demuxer)?)),
      None => Backend::Software(SoftwarePlayback::new(path)?),
    };
    Ok(Self {
      backend,
      frame_number: 0,
      time: Duration::ZERO,
    })
  }

  pub fn next_frame(&mut self, elapsed: Duration) -> Result<Option<Frame>> {
    self.time += elapsed;
    let frame = match &mut self.backend {
      Backend::Gpu(playback) => playback.next_frame(self.time)?.map(Frame::Gpu),
      Backend::Software(playback) => playback.next_frame(self.time)?.map(Frame::Software),
    };
    if frame.is_some() {
      self.frame_number += 1;
    }
    Ok(frame)
  }
}

struct GpuPlayback {
  demuxer: Mp4Demuxer,
  decoder: WgpuTexturesDecoderH264,
  frame_receiver: Receiver<OutputFrame<VideoTexture>>,
  eof: bool,
  decoded_frames: VecDeque<OutputFrame<VideoTexture>>,
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
    })
  }

  fn next_frame(&mut self, time: Duration) -> Result<Option<OutputFrame<VideoTexture>>> {
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
