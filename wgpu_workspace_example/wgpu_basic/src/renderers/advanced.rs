use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
  position: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
  center: [f32; 2],
  scale: [f32; 2],
  color: [f32; 3],
}

impl Vertex {
  const ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];
  fn layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
      array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
      step_mode: wgpu::VertexStepMode::Vertex,
      attributes: &Self::ATTRIBUTES,
    }
  }
}

impl Instance {
  const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![1 => Float32x2, 2 => Float32x2, 3 => Float32x3];
  fn layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
      array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
      step_mode: wgpu::VertexStepMode::Instance,
      attributes: &Self::ATTRIBUTES,
    }
  }
}

pub struct AdvancedRenderer {
  bundle: wgpu::RenderBundle,
}

impl AdvancedRenderer {
  pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: Some("advanced shader"),
      source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/advanced.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label: Some("advanced pipeline layout"),
      bind_group_layouts: &[],
      immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("advanced instanced pipeline"),
      layout: Some(&pipeline_layout),
      vertex: wgpu::VertexState {
        module: &shader,
        entry_point: Some("vs_main"),
        buffers: &[Some(Vertex::layout()), Some(Instance::layout())],
        compilation_options: Default::default(),
      },
      fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: Some("fs_main"),
        targets: &[Some(wgpu::ColorTargetState {
          format,
          blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
    let instances = [
      Instance {
        center: [-0.52, 0.40],
        scale: [0.34, 0.25],
        color: [0.96, 0.28, 0.24],
      },
      Instance {
        center: [0.42, 0.40],
        scale: [0.28, 0.36],
        color: [0.18, 0.78, 0.58],
      },
      Instance {
        center: [-0.42, -0.42],
        scale: [0.26, 0.22],
        color: [0.98, 0.72, 0.18],
      },
      Instance {
        center: [0.46, -0.40],
        scale: [0.38, 0.20],
        color: [0.36, 0.52, 0.98],
      },
    ];
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("advanced quad vertices"),
      contents: bytemuck::cast_slice(&vertices),
      usage: wgpu::BufferUsages::VERTEX,
    });
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("advanced instances"),
      contents: bytemuck::cast_slice(&instances),
      usage: wgpu::BufferUsages::VERTEX,
    });
    let mut encoder = device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
      label: Some("advanced render bundle encoder"),
      color_formats: &[Some(format)],
      depth_stencil: None,
      sample_count: 1,
      multiview: None,
    });
    encoder.set_pipeline(&pipeline);
    encoder.set_vertex_buffer(0, vertex_buffer.slice(..));
    encoder.set_vertex_buffer(1, instance_buffer.slice(..));
    encoder.draw(0 .. vertices.len() as u32, 0 .. instances.len() as u32);
    let bundle = encoder.finish(&wgpu::RenderBundleDescriptor {
      label: Some("advanced render bundle"),
    });
    Self { bundle }
  }

  pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
    pass.execute_bundles(std::iter::once(&self.bundle));
  }
}
