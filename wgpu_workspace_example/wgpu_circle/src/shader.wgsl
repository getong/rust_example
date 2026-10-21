struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

struct Screen {
  resolution: vec2<f32>,
  _padding: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> screen: Screen;

@vertex
fn vs_main(@location(0) position: vec2<f32>) -> VertexOutput {
  var output: VertexOutput;
  output.position = vec4<f32>(position, 0.0, 1.0);
  // 将矩形顶点坐标映射到 [-1, 1]，方便在片元阶段计算圆心距离。
  output.uv = position;
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  let aspect_corrected_uv = vec2<f32>(
    input.uv.x * screen.resolution.x / screen.resolution.y,
    input.uv.y,
  );
  let distance_from_center = length(aspect_corrected_uv);
  let edge = fwidth(distance_from_center);
  let alpha = 1.0 - smoothstep(1.0 - edge, 1.0 + edge, distance_from_center);

  if (alpha <= 0.0) {
    discard;
  }

  return vec4<f32>(0.12, 0.62, 0.95, alpha);
}
