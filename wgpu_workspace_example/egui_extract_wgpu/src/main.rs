#![recursion_limit = "512"]

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use eframe::{egui, egui_wgpu, wgpu};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuVertex {
  position: [f32; 2],
  color: [f32; 4],
}

impl GpuVertex {
  const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];

  fn layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
      array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
      step_mode: wgpu::VertexStepMode::Vertex,
      attributes: &Self::ATTRIBUTES,
    }
  }
}

#[derive(Clone)]
struct PrimitiveData {
  points: Vec<GpuVertex>,
  lines: Vec<GpuVertex>,
  triangles: Vec<GpuVertex>,
}

struct GpuResources {
  point_pipeline: wgpu::RenderPipeline,
  line_pipeline: wgpu::RenderPipeline,
  triangle_pipeline: wgpu::RenderPipeline,
  vertex_buffer: wgpu::Buffer,
}

struct DrawCallback {
  data: Arc<PrimitiveData>,
}

impl egui_wgpu::CallbackTrait for DrawCallback {
  fn prepare(
    &self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    _screen_descriptor: &egui_wgpu::ScreenDescriptor,
    _egui_encoder: &mut wgpu::CommandEncoder,
    callback_resources: &mut egui_wgpu::CallbackResources,
  ) -> Vec<wgpu::CommandBuffer> {
    let resources = callback_resources
      .get_mut::<GpuResources>()
      .expect("GPU resources were not initialized");
    let vertices = self
      .data
      .points
      .iter()
      .chain(&self.data.lines)
      .chain(&self.data.triangles)
      .copied()
      .collect::<Vec<_>>();
    let size = (vertices.len() * std::mem::size_of::<GpuVertex>()) as wgpu::BufferAddress;
    if resources.vertex_buffer.size() < size.max(1) {
      resources.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("egui extracted primitive vertices"),
        size: size.max(1),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
      });
    }
    if !vertices.is_empty() {
      queue.write_buffer(&resources.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
    }
    Vec::new()
  }

  fn paint(
    &self,
    info: egui::PaintCallbackInfo,
    render_pass: &mut wgpu::RenderPass<'static>,
    callback_resources: &egui_wgpu::CallbackResources,
  ) {
    let resources = callback_resources
      .get::<GpuResources>()
      .expect("GPU resources were not initialized");
    let clip = info.clip_rect_in_pixels();
    render_pass.set_scissor_rect(
      clip.left_px.max(0) as u32,
      clip.top_px.max(0) as u32,
      clip.width_px.max(0) as u32,
      clip.height_px.max(0) as u32,
    );
    render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));

    let mut offset = 0;
    draw(
      render_pass,
      &resources.point_pipeline,
      &mut offset,
      self.data.points.len(),
    );
    draw(
      render_pass,
      &resources.line_pipeline,
      &mut offset,
      self.data.lines.len(),
    );
    draw(
      render_pass,
      &resources.triangle_pipeline,
      &mut offset,
      self.data.triangles.len(),
    );
  }
}

fn draw(
  render_pass: &mut wgpu::RenderPass<'static>,
  pipeline: &wgpu::RenderPipeline,
  offset: &mut usize,
  count: usize,
) {
  if count > 0 {
    render_pass.set_pipeline(pipeline);
    render_pass.draw(*offset as u32 .. (*offset + count) as u32, 0 .. 1);
    *offset += count;
  }
}

fn make_pipeline(
  device: &wgpu::Device,
  format: wgpu::TextureFormat,
  topology: wgpu::PrimitiveTopology,
) -> wgpu::RenderPipeline {
  let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("egui extracted primitive shader"),
    source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
  });
  let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
    label: Some("egui extracted primitive pipeline layout"),
    bind_group_layouts: &[],
    immediate_size: 0,
  });
  device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    label: Some("egui extracted primitive pipeline"),
    layout: Some(&layout),
    vertex: wgpu::VertexState {
      module: &module,
      entry_point: Some("vs_main"),
      buffers: &[Some(GpuVertex::layout())],
      compilation_options: Default::default(),
    },
    fragment: Some(wgpu::FragmentState {
      module: &module,
      entry_point: Some("fs_main"),
      targets: &[Some(wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
      })],
      compilation_options: Default::default(),
    }),
    primitive: wgpu::PrimitiveState {
      topology,
      ..Default::default()
    },
    depth_stencil: None,
    multisample: wgpu::MultisampleState::default(),
    multiview_mask: None,
    cache: None,
  })
}

fn egui_mesh_to_triangles(
  ctx: &egui::Context,
  rect: egui::Rect,
  pixels_per_point: f32,
) -> Vec<GpuVertex> {
  let shape = egui::Shape::circle_filled(
    rect.center(),
    rect.width() * 0.28,
    egui::Color32::from_rgb(245, 180, 65),
  );
  let primitives = ctx.tessellate(
    vec![egui::epaint::ClippedShape {
      clip_rect: rect,
      shape,
      transform_after_tessellation: egui::emath::TSTransform::IDENTITY,
    }],
    pixels_per_point,
  );
  primitives
    .into_iter()
    .filter_map(|primitive| match primitive.primitive {
      egui::epaint::Primitive::Mesh(mesh) => Some(mesh),
      egui::epaint::Primitive::Callback(_) => None,
    })
    .flat_map(|mesh| {
      mesh.indices.into_iter().map(move |index| {
        let vertex = mesh.vertices[index as usize];
        GpuVertex {
          position: [
            ((vertex.pos.x - rect.left()) / rect.width()) * 2.0 - 1.0,
            1.0 - ((vertex.pos.y - rect.top()) / rect.height()) * 2.0,
          ],
          color: [
            vertex.color.r() as f32 / 255.0,
            vertex.color.g() as f32 / 255.0,
            vertex.color.b() as f32 / 255.0,
            vertex.color.a() as f32 / 255.0,
          ],
        }
      })
    })
    .collect()
}

struct DemoApp {
  primitives: Arc<PrimitiveData>,
}

impl DemoApp {
  fn new(cc: &eframe::CreationContext<'_>) -> Self {
    let render_state = cc
      .wgpu_render_state
      .as_ref()
      .expect("WGPU renderer required");
    let resources = GpuResources {
      point_pipeline: make_pipeline(
        &render_state.device,
        render_state.target_format,
        wgpu::PrimitiveTopology::PointList,
      ),
      line_pipeline: make_pipeline(
        &render_state.device,
        render_state.target_format,
        wgpu::PrimitiveTopology::LineList,
      ),
      triangle_pipeline: make_pipeline(
        &render_state.device,
        render_state.target_format,
        wgpu::PrimitiveTopology::TriangleList,
      ),
      vertex_buffer: render_state.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("egui extracted primitive vertices"),
        size: 1,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
      }),
    };
    render_state
      .renderer
      .write()
      .callback_resources
      .insert(resources);
    Self {
      primitives: Arc::new(PrimitiveData {
        points: Vec::new(),
        lines: Vec::new(),
        triangles: Vec::new(),
      }),
    }
  }
}

impl eframe::App for DemoApp {
  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    let ctx = ui.ctx().clone();
    ui.vertical(|ui| {
      ui.heading("egui -> wgpu primitives");
      ui.label("tessellate -> Mesh -> wgpu vertex buffer; points and lines use the same callback");
      let (rect, _) = ui.allocate_exact_size(egui::vec2(560.0, 360.0), egui::Sense::hover());
      let triangles = egui_mesh_to_triangles(&ctx, rect, ctx.pixels_per_point());
      self.primitives = Arc::new(PrimitiveData {
        points: vec![
          GpuVertex {
            position: [-0.8, -0.75],
            color: [0.95, 0.3, 0.2, 1.0],
          },
          GpuVertex {
            position: [-0.55, -0.55],
            color: [0.95, 0.3, 0.2, 1.0],
          },
          GpuVertex {
            position: [-0.3, -0.75],
            color: [0.95, 0.3, 0.2, 1.0],
          },
        ],
        lines: vec![
          GpuVertex {
            position: [0.35, -0.7],
            color: [0.2, 0.85, 0.95, 1.0],
          },
          GpuVertex {
            position: [0.75, -0.2],
            color: [0.2, 0.85, 0.95, 1.0],
          },
          GpuVertex {
            position: [0.75, -0.2],
            color: [0.2, 0.85, 0.95, 1.0],
          },
          GpuVertex {
            position: [0.35, 0.45],
            color: [0.2, 0.85, 0.95, 1.0],
          },
        ],
        triangles,
      });
      ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        DrawCallback {
          data: Arc::clone(&self.primitives),
        },
      ));
    });
    ctx.request_repaint();
  }
}

fn main() -> eframe::Result {
  let options = eframe::NativeOptions {
    renderer: eframe::Renderer::Wgpu,
    viewport: egui::ViewportBuilder::default().with_inner_size([760.0, 520.0]),
    ..Default::default()
  };
  eframe::run_native(
    "egui extract wgpu",
    options,
    Box::new(|cc| Ok(Box::new(DemoApp::new(cc)))),
  )
}
