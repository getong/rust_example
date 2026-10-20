# wgpu basic / advanced rendering

This example targets `wgpu 30.0.1` and `winit 0.30.13`.

## Run

```text
cargo run
```

- `1`: direct/basic vertex-buffer rendering
- `2` or `Space`: switch rendering mode
- `Esc`: exit

## Modules

- `src/renderers/basic.rs`: direct `RenderPass` commands using one colored vertex buffer
  with `TriangleList`, `LineList`, and `PointList` primitive topologies.
- `src/renderers/advanced.rs`: instanced quads recorded into a `RenderBundle`.
- `src/shaders/`: one WGSL shader per renderer.
- `src/main.rs`: shared window, surface, resize, and frame lifecycle.

wgpu intentionally exposes a portable GPU abstraction rather than a 2D shape API. The basic
mode demonstrates triangle, line, and point primitive units directly; the advanced path packages
repeated quad state and instance data behind higher-level renderer code.
