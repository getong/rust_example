mod renderers;

use std::sync::Arc;

use renderers::{AdvancedRenderer, BasicRenderer, RenderMode};
use winit::{
  application::ApplicationHandler,
  event::{ElementState, KeyEvent, WindowEvent},
  event_loop::{ActiveEventLoop, EventLoop},
  keyboard::{KeyCode, PhysicalKey},
  window::{Window, WindowId},
};

struct State {
  surface: wgpu::Surface<'static>,
  device: wgpu::Device,
  queue: wgpu::Queue,
  config: wgpu::SurfaceConfiguration,
  size: winit::dpi::PhysicalSize<u32>,
  basic: BasicRenderer,
  advanced: AdvancedRenderer,
  mode: RenderMode,
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
        label: Some("wgpu-basic device"),
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
    let basic = BasicRenderer::new(&device, config.format);
    let advanced = AdvancedRenderer::new(&device, config.format);

    Self {
      surface,
      device,
      queue,
      config,
      size,
      basic,
      advanced,
      mode: RenderMode::Basic,
    }
  }

  fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
    if size.width == 0 || size.height == 0 {
      return;
    }
    self.size = size;
    self.config.width = size.width;
    self.config.height = size.height;
    self.surface.configure(&self.device, &self.config);
  }

  fn render(&mut self) -> bool {
    let frame = match self.surface.get_current_texture() {
      wgpu::CurrentSurfaceTexture::Success(frame)
      | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
      wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
        self.surface.configure(&self.device, &self.config);
        return false;
      }
      wgpu::CurrentSurfaceTexture::Timeout
      | wgpu::CurrentSurfaceTexture::Occluded
      | wgpu::CurrentSurfaceTexture::Validation => return false,
    };
    let view = frame
      .texture
      .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = self
      .device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("frame command encoder"),
      });
    {
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("main render pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: &view,
          depth_slice: None,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color {
              r: 0.035,
              g: 0.055,
              b: 0.09,
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
      match self.mode {
        RenderMode::Basic => self.basic.draw(&mut pass),
        RenderMode::Advanced => self.advanced.draw(&mut pass),
      }
    }
    self.queue.submit(std::iter::once(encoder.finish()));
    self.queue.present(frame);
    true
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
        .create_window(Window::default_attributes().with_title("wgpu 基础 / 高级图形 API"))
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
      WindowEvent::Resized(size) => state.resize(size),
      WindowEvent::KeyboardInput {
        event:
          KeyEvent {
            physical_key: PhysicalKey::Code(code),
            state: ElementState::Pressed,
            ..
          },
        ..
      } => match code {
        KeyCode::Digit1 => state.mode = RenderMode::Basic,
        KeyCode::Digit2 | KeyCode::Space => state.mode = state.mode.toggle(),
        KeyCode::Escape => event_loop.exit(),
        _ => {}
      },
      WindowEvent::RedrawRequested => {
        state.render();
      }
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
