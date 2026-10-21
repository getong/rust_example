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
  @location(1) normal: vec3<f32>,
  @location(2) world_position: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32, @builtin(instance_index) instance: u32) -> VertexOutput {
  let face = index / 6u;
  let corner = index % 6u;
  let uv = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0)
  )[corner];
  let normals = array<vec3<f32>, 6>(
    vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0),
    vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0),
    vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, -1.0)
  );
  let face_colors = array<vec3<f32>, 6>(
    vec3<f32>(0.05, 0.70, 1.00), vec3<f32>(0.10, 0.25, 0.95),
    vec3<f32>(0.90, 0.15, 0.70), vec3<f32>(0.10, 0.95, 0.55),
    vec3<f32>(1.00, 0.30, 0.08), vec3<f32>(1.00, 0.80, 0.08)
  );
  var local: vec3<f32>;
  if (face == 0u) { local = vec3<f32>(1.0, uv.x, uv.y); }
  else if (face == 1u) { local = vec3<f32>(-1.0, uv.x, uv.y); }
  else if (face == 2u) { local = vec3<f32>(uv.x, 1.0, uv.y); }
  else if (face == 3u) { local = vec3<f32>(uv.x, -1.0, uv.y); }
  else if (face == 4u) { local = vec3<f32>(uv.x, uv.y, 1.0); }
  else { local = vec3<f32>(uv.x, uv.y, -1.0); }

  let instance_f = f32(instance);
  var scale = 1.0;
  var center = vec3<f32>(0.0, 0.0, 0.0);
  if (instance == 1u) {
    scale = 0.42;
    center = vec3<f32>(sin(uniforms.time * 0.9) * 2.5, cos(uniforms.time * 0.7) * 0.8, 0.2);
  } else if (instance == 2u) {
    scale = 0.30;
    center = vec3<f32>(cos(uniforms.time * 0.8) * 2.1, -1.2, sin(uniforms.time * 0.8) * 1.5);
  }
  let spin = uniforms.time * (0.65 + instance_f * 0.18);
  let model = rotate_y(rotate_x(local * scale, spin * 0.73), spin) + center;
  let transformed_normal = normalize(rotate_y(rotate_x(normals[face], spin * 0.73), spin));
  let camera_z = 7.0 - model.z;
  let focal_length = 1.35;
  var output: VertexOutput;
  output.position = vec4<f32>(model.x * focal_length / (uniforms.aspect * camera_z), model.y * focal_length / camera_z, camera_z / 20.0, 1.0);
  output.color = face_colors[face];
  output.normal = transformed_normal;
  output.world_position = model;
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  let light_direction = normalize(vec3<f32>(-0.45, 0.85, 0.65));
  let view_direction = normalize(vec3<f32>(0.0, 0.0, 7.0) - input.world_position);
  let half_direction = normalize(light_direction + view_direction);
  let diffuse = max(dot(input.normal, light_direction), 0.0);
  let specular = pow(max(dot(input.normal, half_direction), 0.0), 48.0);
  let rim = pow(1.0 - max(dot(input.normal, view_direction), 0.0), 3.0);
  let lit = input.color * (0.16 + diffuse * 0.84) + vec3<f32>(1.0, 0.9, 0.7) * specular * 0.85 + vec3<f32>(0.1, 0.5, 1.0) * rim * 0.35;
  return vec4<f32>(lit, 1.0);
}

fn rotate_x(p: vec3<f32>, angle: f32) -> vec3<f32> {
  let c = cos(angle); let s = sin(angle);
  return vec3<f32>(p.x, p.y * c - p.z * s, p.y * s + p.z * c);
}

fn rotate_y(p: vec3<f32>, angle: f32) -> vec3<f32> {
  let c = cos(angle); let s = sin(angle);
  return vec3<f32>(p.x * c + p.z * s, p.y, -p.x * s + p.z * c);
}
