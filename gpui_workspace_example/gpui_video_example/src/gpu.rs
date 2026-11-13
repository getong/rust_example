use anyhow::{Context, Result};
use gpu_video::{VideoAdapterExt, parameters::VideoDeviceDescriptor};

#[derive(Clone)]
pub struct GpuContext {
  pub device: wgpu::Device,
  pub queue: wgpu::Queue,
}

impl GpuContext {
  pub async fn new() -> Result<Self> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = instance
      .enumerate_adapters(wgpu::Backends::default())
      .await
      .into_iter()
      .find(|adapter| {
        adapter
          .video_adapter_info()
          .is_some_and(|info| info.decode_capabilities.h264.is_some())
      })
      .context("No GPU adapter supports H.264 decoding")?;
    let (device, queue) = adapter
      .request_device_with_video_support(&VideoDeviceDescriptor::default())
      .context("Failed to create the video decoding device")?;
    Ok(Self { device, queue })
  }
}
