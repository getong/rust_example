use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
  position: [f32; 2],
  color: [f32; 3],
}

impl Vertex {
  const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3];

  fn layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
      array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
      step_mode: wgpu::VertexStepMode::Vertex,
      attributes: &Self::ATTRIBUTES,
    }
  }
}

pub struct BasicRenderer {
  pipeline: wgpu::RenderPipeline,
  vertex_buffer: wgpu::Buffer,
  vertex_count: u32,
}

impl BasicRenderer {
  pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("basic shader"),
      source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/basic.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label: Some("basic pipeline layout"),
      bind_group_layouts: &[],
      immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("basic triangle pipeline"),
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
          format,
          blend: Some(wgpu::BlendState::REPLACE),
          write_mask: wgpu::ColorWrites::ALL,
        })],
        compilation_options: Default::default(),
      }),
      primitive: wgpu::PrimitiveState::default(),
      depth_stencil: None,
      multisample: wgpu::MultisampleState::default(),
      multiview_mask: None,
      cache: None,
    });
    let vertices = [
      Vertex {
        position: [-0.78, -0.72],
        color: [0.12, 0.48, 0.85],
      },
      Vertex {
        position: [0.02, -0.72],
        color: [0.12, 0.48, 0.85],
      },
      Vertex {
        position: [0.02, 0.02],
        color: [0.12, 0.48, 0.85],
      },
      Vertex {
        position: [-0.78, -0.72],
        color: [0.12, 0.48, 0.85],
      },
      Vertex {
        position: [0.02, 0.02],
        color: [0.12, 0.48, 0.85],
      },
      Vertex {
        position: [-0.78, 0.02],
        color: [0.12, 0.48, 0.85],
      },
      Vertex {
        position: [-0.88, 0.02],
        color: [0.95, 0.38, 0.16],
      },
      Vertex {
        position: [-0.38, 0.66],
        color: [1.0, 0.72, 0.18],
      },
      Vertex {
        position: [0.12, 0.02],
        color: [0.95, 0.38, 0.16],
      },
    ];
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("basic vertices"),
      contents: bytemuck::cast_slice(&vertices),
      usage: wgpu::BufferUsages::VERTEX,
    });
    Self {
      pipeline,
      vertex_buffer,
      vertex_count: vertices.len() as u32,
    }
  }

  pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
    pass.set_pipeline(&self.pipeline);
    pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
    pass.draw(0 .. self.vertex_count, 0 .. 1);
  }
}
