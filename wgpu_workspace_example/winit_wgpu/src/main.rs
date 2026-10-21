//! A small multi-window `winit` + `wgpu` example.
//!
//! The important relationship is:
//! - `winit` owns the event loop and native windows.
//! - `wgpu` owns the GPU device/queue and renders into one `Surface` per window.
//! - A window resize changes only that window's `SurfaceConfiguration`.

use std::{borrow::Cow, collections::HashMap, error::Error, sync::Arc, time::Instant};

use wgpu::{CompositeAlphaMode, Instance, Surface, SurfaceConfiguration, TextureUsages};
use winit::{
  application::ApplicationHandler,
  dpi::PhysicalSize,
  event::{ElementState, KeyEvent, WindowEvent},
  event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
  keyboard::{KeyCode, PhysicalKey},
  window::{Window, WindowAttributes, WindowId},
};

struct WindowState {
  window: Arc<Window>,
  surface: Surface<'static>,
  config: SurfaceConfiguration,
}

struct Renderer {
  instance: Instance,
  adapter: wgpu::Adapter,
  device: wgpu::Device,
  queue: wgpu::Queue,
  render_pipeline: wgpu::RenderPipeline,
  time_buffer: wgpu::Buffer,
  time_bind_group: wgpu::BindGroup,
  start_time: Instant,
}

impl Renderer {
  async fn new(window: &Arc<Window>) -> Result<(Self, WindowState), Box<dyn Error>> {
    let instance = Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let surface = instance.create_surface(window.clone())?;
    let adapter = instance
      .request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
        apply_limit_buckets: false,
      })
      .await?;
    let capabilities = surface.get_capabilities(&adapter);
    let surface_format = capabilities
      .formats
      .iter()
      .copied()
      .find(wgpu::TextureFormat::is_srgb)
      .or_else(|| capabilities.formats.first().copied())
      .ok_or("window surface has no supported texture formats")?;

    let (device, queue) = adapter
      .request_device(&wgpu::DeviceDescriptor {
        label: Some("shared-device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
      })
      .await?;

    // The shader generates the geometry from vertex_index, so no vertex array is
    // needed. The uniform contains time and aspect ratio for animation/layout.
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("animated-shapes-shader"),
      source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(
        r#"
struct Uniforms {
  time: f32,
  aspect: f32,
  _padding: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
  // Six vertices make a rotating square. The last three vertices make a
  // second triangle, which visibly demonstrates that the GPU is drawing data.
  var positions = array<vec2<f32>, 9>(
    vec2<f32>(-0.62, -0.62), vec2<f32>( 0.62, -0.62), vec2<f32>( 0.62,  0.62),
    vec2<f32>(-0.62, -0.62), vec2<f32>( 0.62,  0.62), vec2<f32>(-0.62,  0.62),
    vec2<f32>( 0.00,  0.84), vec2<f32>( 0.78, -0.48), vec2<f32>(-0.78, -0.48),
  );
  var colors = array<vec3<f32>, 9>(
    vec3<f32>(0.10, 0.80, 1.00), vec3<f32>(0.20, 0.40, 1.00), vec3<f32>(0.85, 0.20, 0.90),
    vec3<f32>(0.10, 0.80, 1.00), vec3<f32>(0.85, 0.20, 0.90), vec3<f32>(0.10, 1.00, 0.55),
    vec3<f32>(1.00, 0.35, 0.15), vec3<f32>(1.00, 0.85, 0.10), vec3<f32>(1.00, 0.20, 0.35),
  );

  let angle = uniforms.time * 0.8;
  let c = cos(angle);
  let s = sin(angle);
  let p = positions[index];
  let rotated = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);

  var output: VertexOutput;
  output.position = vec4<f32>(rotated.x / uniforms.aspect, rotated.y, 0.0, 1.0);
  output.color = colors[index] * (0.75 + 0.25 * sin(uniforms.time + f32(index)));
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  return vec4<f32>(input.color, 1.0);
}
"#,
      )),
    });
    let time_buffer = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("animation-uniform-buffer"),
      size: 16,
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: Some("animation-bind-group-layout"),
      entries: &[wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
          ty: wgpu::BufferBindingType::Uniform,
          has_dynamic_offset: false,
          min_binding_size: None,
        },
        count: None,
      }],
    });
    let time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("animation-bind-group"),
      layout: &bind_group_layout,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: time_buffer.as_entire_binding(),
      }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label: Some("animated-shapes-pipeline-layout"),
      bind_group_layouts: &[Some(&bind_group_layout)],
      immediate_size: 0,
    });
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("animated-shapes-pipeline"),
      layout: Some(&pipeline_layout),
      vertex: wgpu::VertexState {
        module: &shader,
        entry_point: Some("vs_main"),
        buffers: &[],
        compilation_options: wgpu::PipelineCompilationOptions::default(),
      },
      fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: Some("fs_main"),
        targets: &[Some(wgpu::ColorTargetState {
          format: surface_format,
          blend: Some(wgpu::BlendState::REPLACE),
          write_mask: wgpu::ColorWrites::ALL,
        })],
        compilation_options: wgpu::PipelineCompilationOptions::default(),
      }),
      primitive: wgpu::PrimitiveState::default(),
      depth_stencil: None,
      multisample: wgpu::MultisampleState::default(),
      multiview_mask: None,
      cache: None,
    });

    let renderer = Self {
      instance,
      adapter,
      device,
      queue,
      render_pipeline,
      time_buffer,
      time_bind_group,
      start_time: Instant::now(),
    };
    let state = renderer.create_window_state(window.clone(), surface)?;
    Ok((renderer, state))
  }

  fn create_window_state(
    &self,
    window: Arc<Window>,
    surface: Surface<'static>,
  ) -> Result<WindowState, Box<dyn Error>> {
    let size = window.inner_size();
    let capabilities = surface.get_capabilities(&self.adapter);
    let format = capabilities
      .formats
      .iter()
      .copied()
      .find(wgpu::TextureFormat::is_srgb)
      .or_else(|| capabilities.formats.first().copied())
      .ok_or("window surface has no supported texture formats")?;
    let present_mode = capabilities
      .present_modes
      .iter()
      .copied()
      .find(|mode| *mode == wgpu::PresentMode::Fifo)
      .unwrap_or(wgpu::PresentMode::Fifo);
    let alpha_mode = capabilities
      .alpha_modes
      .first()
      .copied()
      .unwrap_or(CompositeAlphaMode::Auto);
    let config = SurfaceConfiguration {
      usage: TextureUsages::RENDER_ATTACHMENT,
      format,
      color_space: wgpu::SurfaceColorSpace::Auto,
      width: size.width.max(1),
      height: size.height.max(1),
      present_mode,
      alpha_mode,
      view_formats: vec![],
      desired_maximum_frame_latency: 2,
    };
    surface.configure(&self.device, &config);

    Ok(WindowState {
      window,
      surface,
      config,
    })
  }

  fn render(&self, state: &mut WindowState) -> Result<(), wgpu::CurrentSurfaceTexture> {
    let output = match state.surface.get_current_texture() {
      wgpu::CurrentSurfaceTexture::Success(output)
      | wgpu::CurrentSurfaceTexture::Suboptimal(output) => output,
      status => return Err(status),
    };
    let view = output
      .texture
      .create_view(&wgpu::TextureViewDescriptor::default());
    let elapsed = self.start_time.elapsed().as_secs_f32();
    let phase = elapsed * 0.7;
    let aspect = state.config.width as f32 / state.config.height as f32;
    let mut uniform_data = [0_u8; 16];
    uniform_data[0 .. 4].copy_from_slice(&phase.to_ne_bytes());
    uniform_data[4 .. 8].copy_from_slice(&aspect.to_ne_bytes());
    self.queue.write_buffer(&self.time_buffer, 0, &uniform_data);
    let color = wgpu::Color {
      r: 0.08 + f64::from(phase.sin().abs()) * 0.20,
      g: 0.12 + f64::from((phase * 0.8).sin().abs()) * 0.25,
      b: 0.22 + f64::from((phase * 1.3).cos().abs()) * 0.35,
      a: 1.0,
    };
    let mut encoder = self
      .device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("clear-window-encoder"),
      });
    {
      let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("animated-clear-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: &view,
          depth_slice: None,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(color),
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
      });
      drop(_pass);
    }
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
      label: Some("animated-shapes-pass"),
      color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        view: &view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
          load: wgpu::LoadOp::Load,
          store: wgpu::StoreOp::Store,
        },
      })],
      depth_stencil_attachment: None,
      timestamp_writes: None,
      occlusion_query_set: None,
      multiview_mask: None,
    });
    pass.set_pipeline(&self.render_pipeline);
    pass.set_bind_group(0, &self.time_bind_group, &[]);
    pass.draw(0 .. 9, 0 .. 1);
    drop(pass);
    self.queue.submit([encoder.finish()]);
    self.queue.present(output);
    Ok(())
  }
}

struct App {
  renderer: Option<Renderer>,
  windows: HashMap<WindowId, WindowState>,
  next_window_number: usize,
}

impl App {
  fn new() -> Self {
    Self {
      renderer: None,
      windows: HashMap::new(),
      next_window_number: 1,
    }
  }

  fn create_window(&mut self, event_loop: &ActiveEventLoop) {
    let title = format!("winit + wgpu | window {}", self.next_window_number);
    self.next_window_number += 1;
    let attributes = WindowAttributes::default()
      .with_title(title)
      .with_inner_size(PhysicalSize::new(720, 480));
    let window = match event_loop.create_window(attributes) {
      Ok(window) => Arc::new(window),
      Err(error) => {
        eprintln!("failed to create window: {error}");
        return;
      }
    };

    if let Some(renderer) = &self.renderer {
      let surface = match renderer.instance.create_surface(window.clone()) {
        Ok(surface) => surface,
        Err(error) => {
          eprintln!("failed to create surface: {error}");
          return;
        }
      };
      match renderer.create_window_state(window, surface) {
        Ok(state) => {
          self.windows.insert(state.window.id(), state);
        }
        Err(error) => eprintln!("failed to configure new window: {error}"),
      }
      return;
    }

    match pollster::block_on(Renderer::new(&window)) {
      Ok((renderer, state)) => {
        self.windows.insert(state.window.id(), state);
        self.renderer = Some(renderer);
      }
      Err(error) => eprintln!("failed to initialize wgpu: {error}"),
    }
  }

  fn render_window(&mut self, window_id: WindowId) {
    let Some(renderer) = &self.renderer else {
      return;
    };
    let Some(state) = self.windows.get_mut(&window_id) else {
      return;
    };
    match renderer.render(state) {
      Ok(()) => {}
      Err(wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated) => {
        state.surface.configure(&renderer.device, &state.config);
      }
      Err(wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded) => {}
      Err(wgpu::CurrentSurfaceTexture::Validation) => {
        eprintln!("surface validation failed for window {:?}", window_id);
      }
      Err(wgpu::CurrentSurfaceTexture::Success(_))
      | Err(wgpu::CurrentSurfaceTexture::Suboptimal(_)) => unreachable!(),
    }
  }
}

impl ApplicationHandler for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    if self.windows.is_empty() {
      self.create_window(event_loop);
    }
  }

  fn window_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    window_id: WindowId,
    event: WindowEvent,
  ) {
    match event {
      WindowEvent::CloseRequested => {
        self.windows.remove(&window_id);
        if self.windows.is_empty() {
          event_loop.exit();
        }
      }
      WindowEvent::Resized(size) => {
        if let (Some(renderer), Some(state)) =
          (self.renderer.as_ref(), self.windows.get_mut(&window_id))
        {
          state.config.width = size.width.max(1);
          state.config.height = size.height.max(1);
          state.surface.configure(&renderer.device, &state.config);
        }
      }
      WindowEvent::RedrawRequested => self.render_window(window_id),
      WindowEvent::KeyboardInput {
        event:
          KeyEvent {
            physical_key: PhysicalKey::Code(KeyCode::KeyN),
            state: ElementState::Pressed,
            repeat: false,
            ..
          },
        ..
      } => self.create_window(event_loop),
      _ => {}
    }
  }

  fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
    for state in self.windows.values() {
      state.window.request_redraw();
    }
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let event_loop = EventLoop::new()?;
  event_loop.set_control_flow(ControlFlow::Poll);
  event_loop.run_app(&mut App::new())?;
  Ok(())
}
