use std::sync::{Arc, mpsc};

use anyhow::{Context as _, Result};
use gpu_video::{
  OutputFrame, VideoTexture, WgpuNv12ToRgbaConverter,
  parameters::{ColorRange, ColorSpace, WgpuConverterParameters},
};
use gpui_kit::{IntoElement, ObjectFit, RenderImage, Window, div, img, prelude::*};
use image::{Frame as ImageFrame, RgbaImage};

use crate::{gpu::GpuContext, media::MediaInfo, playback::Frame};

struct OutputTexture {
  texture: wgpu::Texture,
  view: wgpu::TextureView,
  readback: wgpu::Buffer,
  bytes_per_row: u32,
}

#[derive(Default)]
pub struct VideoView {
  converter: Option<WgpuNv12ToRgbaConverter>,
  texture: Option<OutputTexture>,
  pending: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
  image: Option<Arc<RenderImage>>,
  color: Option<(ColorSpace, ColorRange)>,
}

impl VideoView {
  pub fn configure(&mut self, info: &MediaInfo) {
    self.color = Some((
      match info.color_space.as_str() {
        "smpte170m" => ColorSpace::BT601Ntsc,
        "bt470bg" => ColorSpace::BT601Pal,
        _ if info.height < 720 && info.color_space == "unknown" => ColorSpace::BT601Ntsc,
        _ => ColorSpace::BT709,
      },
      if info.full_range {
        ColorRange::Full
      } else {
        ColorRange::Limited
      },
    ));
    self.converter = None;
  }

  pub fn snapshot(&self) -> Option<Arc<RenderImage>> {
    self.image.clone()
  }

  fn output_texture(&mut self, device: &wgpu::Device, size: wgpu::Extent3d) {
    if self
      .texture
      .as_ref()
      .is_some_and(|output| output.texture.size() == size)
    {
      return;
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
      label: Some("video RGBA output"),
      size,
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: wgpu::TextureFormat::Rgba8Unorm,
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
      view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let bytes_per_row = (size.width * 4).div_ceil(alignment) * alignment;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("video frame readback"),
      size: u64::from(bytes_per_row) * u64::from(size.height),
      usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
      mapped_at_creation: false,
    });
    self.texture = Some(OutputTexture {
      texture,
      view,
      readback,
      bytes_per_row,
    });
  }

  pub fn update(
    &mut self,
    gpu: Option<&GpuContext>,
    frame: Frame,
    window: &mut Window,
  ) -> Result<()> {
    match frame {
      Frame::Gpu(frame) => self.update_gpu(gpu.context("Missing video GPU device")?, &frame),
      Frame::Software(frame) => {
        self.present(frame.width, frame.height, frame.pixels, false, window)
      }
    }
  }

  fn update_gpu(&mut self, gpu: &GpuContext, frame: &OutputFrame<VideoTexture>) -> Result<()> {
    if self.pending.is_some() {
      return Ok(());
    }
    self.output_texture(&gpu.device, frame.data.size());
    if self.converter.is_none() {
      self.converter = Some(
        WgpuNv12ToRgbaConverter::new(
          &gpu.device,
          WgpuConverterParameters {
            color_space: self.color.map_or(ColorSpace::BT709, |color| color.0),
            color_range: self.color.map_or(ColorRange::Limited, |color| color.1),
          },
        )
        .context("Failed to create the video color converter")?,
      );
    }
    let converter = self
      .converter
      .as_ref()
      .context("Missing video color converter")?;
    let output = self
      .texture
      .as_ref()
      .context("Missing video output texture")?;
    let bind_group = converter
      .create_input_bind_group(frame)
      .context("Failed to convert the decoded video frame")?;
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    converter.convert(&mut encoder, &bind_group, &output.view);
    encoder.copy_texture_to_buffer(
      output.texture.as_image_copy(),
      wgpu::TexelCopyBufferInfo {
        buffer: &output.readback,
        layout: wgpu::TexelCopyBufferLayout {
          offset: 0,
          bytes_per_row: Some(output.bytes_per_row),
          rows_per_image: Some(output.texture.height()),
        },
      },
      output.texture.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    let (sender, receiver) = mpsc::channel();
    output
      .readback
      .slice(..)
      .map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
      });
    self.pending = Some(receiver);
    Ok(())
  }

  pub fn poll(&mut self, gpu: Option<&GpuContext>, window: &mut Window) -> Result<()> {
    let Some(receiver) = &self.pending else {
      return Ok(());
    };
    let gpu = gpu.context("Missing video GPU device")?;
    gpu
      .device
      .poll(wgpu::PollType::Poll)
      .context("Failed to poll video frame readback")?;
    match receiver.try_recv() {
      Ok(result) => {
        self.pending = None;
        result.context("Failed to map the video frame")?;
      }
      Err(mpsc::TryRecvError::Empty) => return Ok(()),
      Err(error) => {
        self.pending = None;
        return Err(error).context("Video frame readback was disconnected");
      }
    }
    let output = self
      .texture
      .as_ref()
      .context("Missing video output texture")?;
    let width = output.texture.width();
    let height = output.texture.height();
    let mapped = output
      .readback
      .slice(..)
      .get_mapped_range()
      .context("Failed to read the mapped video frame")?;
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for row in mapped.chunks_exact(output.bytes_per_row as usize) {
      pixels.extend_from_slice(&row[.. width as usize * 4]);
    }
    drop(mapped);
    output.readback.unmap();
    self.present(width, height, pixels, true, window)
  }

  fn present(
    &mut self,
    width: u32,
    height: u32,
    mut pixels: Vec<u8>,
    rgba: bool,
    window: &mut Window,
  ) -> Result<()> {
    if rgba {
      for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
      }
    }
    let buffer = RgbaImage::from_raw(width, height, pixels).context("Invalid video frame size")?;
    let image = Arc::new(RenderImage::new(vec![ImageFrame::new(buffer)]));
    if let Some(previous) = self.image.replace(image) {
      window
        .drop_image(previous)
        .context("Failed to release the previous video frame")?;
    }
    Ok(())
  }

  pub fn clear(&mut self, window: &mut Window) -> Result<()> {
    self.pending = None;
    if let Some(output) = self.texture.take() {
      output.readback.destroy();
    }
    if let Some(image) = self.image.take() {
      window
        .drop_image(image)
        .context("Failed to release the video frame")?;
    }
    Ok(())
  }

  pub fn has_pending_frame(&self) -> bool {
    self.pending.is_some()
  }

  pub fn size(&self) -> Option<(u32, u32)> {
    self.image.as_ref().map(|image| {
      let size = image.size(0);
      (size.width.0 as u32, size.height.0 as u32)
    })
  }

  pub fn element(&self) -> impl IntoElement {
    div()
      .size_full()
      .flex()
      .items_center()
      .justify_center()
      .when_some(self.image.clone(), |element, image| {
        element.child(img(image).size_full().object_fit(ObjectFit::Contain))
      })
      .when(self.image.is_none(), |element| {
        element.child("Open a video or drop media files here")
      })
  }
}
