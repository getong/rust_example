use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::{
  application::ApplicationHandler,
  event::WindowEvent,
  event_loop::{ActiveEventLoop, EventLoop},
  window::{Window, WindowId},
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
  position: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ScreenUniform {
  resolution: [f32; 2],
  _padding: [f32; 2],
}

impl Vertex {
  const ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];

  const fn layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
      array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
      step_mode: wgpu::VertexStepMode::Vertex,
      attributes: &Self::ATTRIBUTES,
    }
  }
}

struct State {
  surface: wgpu::Surface<'static>,
  device: wgpu::Device,
  queue: wgpu::Queue,
  config: wgpu::SurfaceConfiguration,
  pipeline: wgpu::RenderPipeline,
  screen_bind_group: wgpu::BindGroup,
  screen_buffer: wgpu::Buffer,
  vertex_buffer: wgpu::Buffer,
}

impl State {
  async fn new(window: Arc<Window>) -> Self {
    let size = window.inner_size();
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let surface = instance
      .create_surface(window)
      .expect("failed to create wgpu surface");
    let adapter = instance
      .request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
        apply_limit_buckets: false,
      })
      .await
      .expect("no compatible graphics adapter found");
    let (device, queue) = adapter
      .request_device(&wgpu::DeviceDescriptor {
        label: Some("circle device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
      })
      .await
      .expect("failed to request wgpu device");

    let config = surface
      .get_default_config(&adapter, size.width.max(1), size.height.max(1))
      .expect("surface is not supported by the adapter");
    surface.configure(&device, &config);

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("circle shader"),
      source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });
    let screen_bind_group_layout =
      device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("circle screen bind group layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
          binding: 0,
          visibility: wgpu::ShaderStages::FRAGMENT,
          ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<ScreenUniform>() as u64),
          },
          count: None,
        }],
      });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label: Some("circle pipeline layout"),
      bind_group_layouts: &[Some(&screen_bind_group_layout)],
      immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("circle pipeline"),
      layout: Some(&pipeline_layout),
      vertex: wgpu::VertexState {
        module: &shader,
        entry_point: Some("vs_main"),
        buffers: &[Some(Vertex::layout())],
        compilation_options: Default::default(),
      },
      fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: Some("fs_main"),
        targets: &[Some(wgpu::ColorTargetState {
          format: config.format,
          blend: Some(wgpu::BlendState::ALPHA_BLENDING),
          write_mask: wgpu::ColorWrites::ALL,
        })],
        compilation_options: Default::default(),
      }),
      primitive: wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        ..Default::default()
      },
      depth_stencil: None,
      multisample: wgpu::MultisampleState::default(),
      multiview_mask: None,
      cache: None,
    });

    // 一个矩形由两个三角形组成，片元着色器再把矩形裁剪成圆。
    let vertices = [
      Vertex {
        position: [-1.0, -1.0],
      },
      Vertex {
        position: [1.0, -1.0],
      },
      Vertex {
        position: [1.0, 1.0],
      },
      Vertex {
        position: [-1.0, -1.0],
      },
      Vertex {
        position: [1.0, 1.0],
      },
      Vertex {
        position: [-1.0, 1.0],
      },
    ];
    let screen_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("circle screen uniform"),
      contents: bytemuck::bytes_of(&ScreenUniform {
        resolution: [config.width as f32, config.height as f32],
        _padding: [0.0; 2],
      }),
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let screen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("circle screen bind group"),
      layout: &screen_bind_group_layout,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: screen_buffer.as_entire_binding(),
      }],
    });
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("circle vertices"),
      contents: bytemuck::cast_slice(&vertices),
      usage: wgpu::BufferUsages::VERTEX,
    });

    Self {
      surface,
      device,
      queue,
      config,
      pipeline,
      screen_bind_group,
      screen_buffer,
      vertex_buffer,
    }
  }

  fn resize(&mut self, width: u32, height: u32) {
    if width == 0 || height == 0 {
      return;
    }
    self.config.width = width;
    self.config.height = height;
    self.queue.write_buffer(
      &self.screen_buffer,
      0,
      bytemuck::bytes_of(&ScreenUniform {
        resolution: [width as f32, height as f32],
        _padding: [0.0; 2],
      }),
    );
    self.surface.configure(&self.device, &self.config);
  }

  fn render(&mut self) {
    let frame = match self.surface.get_current_texture() {
      wgpu::CurrentSurfaceTexture::Success(frame)
      | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
      wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
        self.surface.configure(&self.device, &self.config);
        return;
      }
      wgpu::CurrentSurfaceTexture::Timeout
      | wgpu::CurrentSurfaceTexture::Occluded
      | wgpu::CurrentSurfaceTexture::Validation => return,
    };
    let view = frame
      .texture
      .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = self
      .device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("circle command encoder"),
      });
    {
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("circle render pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: &view,
          depth_slice: None,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color {
              r: 0.03,
              g: 0.04,
              b: 0.06,
              a: 1.0,
            }),
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
      });
      pass.set_pipeline(&self.pipeline);
      pass.set_bind_group(0, &self.screen_bind_group, &[]);
      pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      pass.draw(0 .. 6, 0 .. 1);
    }
    self.queue.submit(std::iter::once(encoder.finish()));
    self.queue.present(frame);
  }
}

struct App {
  window: Option<Arc<Window>>,
  state: Option<State>,
}

impl ApplicationHandler for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    if self.window.is_some() {
      return;
    }
    let window = Arc::new(
      event_loop
        .create_window(Window::default_attributes().with_title("wgpu circle"))
        .expect("failed to create window"),
    );
    let state = tokio::task::block_in_place(|| {
      tokio::runtime::Handle::current().block_on(State::new(window.clone()))
    });
    window.request_redraw();
    self.window = Some(window);
    self.state = Some(state);
  }

  fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
    let Some(window) = self.window.as_ref() else {
      return;
    };
    if window.id() != id {
      return;
    }
    let Some(state) = self.state.as_mut() else {
      return;
    };
    match event {
      WindowEvent::CloseRequested => event_loop.exit(),
      WindowEvent::Resized(size) => state.resize(size.width, size.height),
      WindowEvent::RedrawRequested => state.render(),
      _ => {}
    }
    window.request_redraw();
  }

  fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
    if let Some(window) = self.window.as_ref() {
      window.request_redraw();
    }
  }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), winit::error::EventLoopError> {
  let event_loop = EventLoop::new()?;
  event_loop.run_app(&mut App {
    window: None,
    state: None,
  })
}
