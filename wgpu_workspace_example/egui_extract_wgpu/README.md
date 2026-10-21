# egui 转换为 wgpu 基础图元

这个示例演示如何把 egui 的绘制数据接入 wgpu，并使用 wgpu 绘制点、线和三角形。

## 设计目标

egui 的高层图形需要先经过 tessellation，转换为 GPU 可以直接消费的三角形网格：

```text
egui::Shape
    |
    v
Context::tessellate
    |
    v
ClippedPrimitive::Mesh
    |
    v
GpuVertex
    |
    v
wgpu vertex buffer + render pipeline
```

`Context::tessellate` 可以理解为“解压”或“展平”：它把圆、矩形、路径、文字等高层图形转换成三角形 Mesh。

## 与 warbell 思路的对应关系

warbell 使用 egui 的自定义绘制回调，把额外的 wgpu 绘制命令插入 egui 的渲染过程。本示例保留这条核心链路：

1. 在 `CreationContext` 中取得 eframe 创建好的 `RenderState`。
2. 使用 `Device` 创建 shader、pipeline 和 vertex buffer。
3. 将 GPU 资源放入 `egui_wgpu::CallbackResources`。
4. 使用 `egui_wgpu::Callback::new_paint_callback` 创建 egui 绘制回调。
5. 在 `prepare` 阶段上传顶点数据。
6. 在 `paint` 阶段复用 egui 当前的 render pass，发出 wgpu draw 命令。

这样可以让自定义图形和普通 egui 控件共享同一个窗口、surface 和渲染目标。

## 主要数据结构

### `GpuVertex`

```rust
#[repr(C)]
struct GpuVertex {
    position: [f32; 2],
    color: [f32; 4],
}
```

它对应 WGSL 中的两个 vertex attribute：

```text
location 0 -> position: vec2<f32>
location 1 -> color:    vec4<f32>
```

`#[repr(C)]` 和 `bytemuck::Pod` 保证结构体布局可以安全地写入 wgpu buffer。

### `PrimitiveData`

示例把三种图元分开保存：

```rust
struct PrimitiveData {
    points: Vec<GpuVertex>,
    lines: Vec<GpuVertex>,
    triangles: Vec<GpuVertex>,
}
```

原因是 wgpu 的 `PrimitiveTopology` 属于 pipeline 状态。一个 pipeline 不能在绘制时动态切换 `PointList`、`LineList` 和 `TriangleList`，因此示例创建了三套 pipeline。

## egui Mesh 转换

`egui_mesh_to_triangles` 的流程是：

1. 创建一个 egui 圆形 Shape。
2. 使用 `Context::tessellate` 得到 `ClippedPrimitive::Mesh`。
3. 遍历 Mesh index，将索引展开为连续的三角形顶点。
4. 将 egui 屏幕坐标转换为局部 NDC 坐标。
5. 将 `Color32` 转换为 `[f32; 4]`。

egui 使用左上角为原点的屏幕坐标，wgpu shader 使用范围为 `[-1, 1]` 的 NDC 坐标。示例对 X、Y 分别做缩放和偏移，其中 Y 轴需要反转，因为屏幕坐标向下增长，而 NDC 坐标向上增长。

## 回调生命周期

### `prepare`

`prepare` 在 egui 主 render pass 开始前执行，适合做 GPU 数据更新：

- 合并点、线、三角形顶点
- 根据实际数据量扩容 vertex buffer
- 使用 `queue.write_buffer` 上传顶点数据

三种图元在同一个 vertex buffer 中连续存放，绘制时通过 vertex offset 区分它们。

### `paint`

`paint` 发生在 egui 的 render pass 中：

- 设置当前 clip rect 对应的 scissor
- 绑定 vertex buffer
- 绑定点、线或三角形 pipeline
- 使用不同的 vertex range 发出 `draw`

## 三种基础图元

示例创建了三套 pipeline：

```rust
PointList
LineList
TriangleList
```

对应规则如下：

- `PointList`：每个 vertex 是一个点
- `LineList`：每两个 vertex 构成一条独立线段
- `TriangleList`：每三个 vertex 构成一个独立三角形

示例中的 egui 圆形最终会被 tessellate 成三角形；点和线使用手工准备的顶点数据。真实项目可以直接生成点线顶点，也可以先使用 egui Shape 生成 Mesh 后再转换。

## 裁剪区域

egui 的绘制区域使用逻辑点坐标，而 wgpu scissor 使用物理像素。回调通过：

```rust
info.clip_rect_in_pixels()
```

取得物理像素区域，然后调用：

```rust
render_pass.set_scissor_rect(x, y, width, height);
```

这一步可以防止自定义图形绘制到 egui 控件区域之外。

## 运行和检查

```bash
RUSTC_WRAPPER= cargo run
RUSTC_WRAPPER= cargo check --offline
RUSTC_WRAPPER= cargo test --offline --no-run
```

`RUSTC_WRAPPER=` 用于绕过本机不可用的 `sccache` 配置。

## 后续扩展

真实项目通常还需要继续完善：

- 使用 index buffer，避免展开重复顶点
- 为纹理、UV 和字体 atlas 增加 vertex attributes
- 按图元类型、纹理和 pipeline 状态合并 draw call
- 使用 uniform buffer 传入变换矩阵和时间参数
- 增加动态 vertex buffer 的容量管理
- 为线宽、点大小、抗锯齿和透明度增加 shader 支持
- 读取 `FullOutput.shapes` 后统一 tessellate 和提交

需要注意：egui 的默认 Shape 最终主要会变成三角形 Mesh。点和线不是所有 Shape 都能直接保留成 `PointList` 或 `LineList`，如果业务需要严格的点线拓扑，应在自己的图形数据层明确生成对应顶点。
