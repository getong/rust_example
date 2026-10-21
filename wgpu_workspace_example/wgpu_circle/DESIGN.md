# WGPU 圆形绘制设计说明

## 1. 问题背景

`wgpu` 的光栅化管线只直接支持三种基础图元：

- `PointList`：点
- `LineList` / `LineStrip`：线
- `TriangleList` / `TriangleStrip`：三角形

圆不是 GPU 光栅化器的原生图元，因此需要把圆转换成 GPU 能处理的图元，或者在着色器中计算圆形区域。

本项目采用第二种方式：用两个三角形覆盖一个矩形，再在片元着色器中判断每个像素是否位于圆内。

## 2. 总体结构

渲染流程如下：

```text
六个顶点
    |
    v
两个三角形组成矩形
    |
    v
顶点着色器输出 NDC 坐标
    |
    v
光栅化生成矩形覆盖的片元
    |
    v
片元着色器计算到圆心的距离
    |
    +-- 圆外：discard
    |
    +-- 圆内：输出颜色和透明度
```

对应代码位于：

- Rust 渲染初始化与绘制：[src/main.rs](src/main.rs)
- WGSL 顶点和片元着色器：[src/shader.wgsl](src/shader.wgsl)

## 3. 为什么使用两个三角形

顶点数据是一个覆盖整个 NDC 区域的矩形：

```text
(-1,  1) +-------------+ (1,  1)
         |       /     |
         |     /       |
         |   /         |
(-1, -1) +-------------+ (1, -1)
```

矩形由两个 `TriangleList` 三角形组成：

```text
[-1, -1], [1, -1], [1, 1]
[-1, -1], [1, 1], [-1, 1]
```

这样做的优点是：

1. 顶点数量固定，不需要根据圆的精度动态生成大量顶点。
2. 圆的边缘质量由片元着色器决定，而不是由多边形边数决定。
3. 调整圆的半径、颜色或边缘效果时，不需要重建顶点缓冲区。

另一种常见方案是生成一个三角形扇：中心点连接多个圆周点。这种方案实现简单，但本质上是多边形近似，边数较少时会看到明显棱角。

## 4. 顶点着色器

顶点着色器直接把二维顶点坐标作为裁剪空间坐标：

```wgsl
output.position = vec4<f32>(position, 0.0, 1.0);
output.uv = position;
```

这里的坐标范围是 `[-1, 1]`：

- `(-1, -1)` 对应窗口左下角
- `(1, 1)` 对应窗口右上角
- `(0, 0)` 对应窗口中心

同时把坐标传给片元着色器。光栅化器会对三角形内部的顶点属性进行插值，因此每个片元都能得到自己相对于窗口中心的位置。

## 5. 片元着色器中的圆方程

以窗口中心为圆心，半径为 `1`，圆的判定条件是：

```text
x² + y² <= r²
```

也可以写成向量长度形式：

```text
length(position) <= radius
```

当前实现使用：

```wgsl
let distance_from_center = length(aspect_corrected_uv);
```

当片元在圆外时丢弃：

```wgsl
if (alpha <= 0.0) {
  discard;
}
```

因此，虽然光栅化器实际处理的是矩形，最终写入颜色的区域却是圆形。

## 6. 抗锯齿

如果直接使用二值判断，圆的边缘会在“完全保留”和“完全丢弃”之间突然切换，容易产生锯齿。

当前实现使用 `fwidth` 估算一个片元范围内距离值的变化，并使用 `smoothstep` 生成平滑透明度：

```wgsl
let edge = fwidth(distance_from_center);
let alpha = 1.0 - smoothstep(
  1.0 - edge,
  1.0 + edge,
  distance_from_center,
);
```

可以把它理解为：

- 圆内区域：`alpha` 接近 `1`
- 圆外区域：`alpha` 接近 `0`
- 圆边缘：`alpha` 在 `0` 到 `1` 之间平滑过渡

管线使用 `wgpu::BlendState::ALPHA_BLENDING`，因此边缘透明度会正确混合到背景颜色上。

## 7. 宽高比校正

NDC 的 `x` 和 `y` 都在 `[-1, 1]`，但窗口的像素宽度和高度通常不同。如果直接计算：

```wgsl
length(input.uv)
```

在宽屏窗口中，圆可能显示成椭圆，因为一个 NDC 单位对应的横向像素数和纵向像素数不同。

因此 Rust 侧通过 uniform 传入窗口分辨率：

```rust
struct ScreenUniform {
  resolution: [f32; 2],
  _padding: [f32; 2],
}
```

片元着色器只对 `x` 分量进行宽高比修正：

```wgsl
let aspect_corrected_uv = vec2<f32>(
  input.uv.x * screen.resolution.x / screen.resolution.y,
  input.uv.y,
);
```

窗口尺寸变化时，`resize` 通过 `queue.write_buffer` 更新 uniform，保证圆在不同窗口尺寸下仍保持正确比例。

## 8. Pipeline 配置

渲染管线的关键配置如下：

```rust
primitive: wgpu::PrimitiveState {
  topology: wgpu::PrimitiveTopology::TriangleList,
  ..Default::default()
}
```

这里明确告诉 `wgpu`：顶点按每三个一组解释为三角形。

颜色目标启用了透明混合：

```rust
blend: Some(wgpu::BlendState::ALPHA_BLENDING)
```

pipeline 还绑定了一个 fragment-stage uniform，用于传递屏幕分辨率。

## 9. Tokio 的初始化流程

`wgpu::Adapter::request_device` 是异步操作，而 `winit` 的 `ApplicationHandler::resumed` 回调是同步的。

程序入口使用 Tokio 多线程 runtime：

```rust
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), winit::error::EventLoopError> {
  // ...
}
```

在同步的 `resumed` 回调中，通过 Tokio 当前 runtime 等待 GPU 初始化：

```rust
let state = tokio::task::block_in_place(|| {
  tokio::runtime::Handle::current().block_on(State::new(window.clone()))
});
```

使用多线程 runtime 是必要的，因为 `block_in_place` 不能用于 Tokio 的单线程 runtime。

## 10. 与三角形扇方案的比较

| 方案 | 优点 | 缺点 |
| --- | --- | --- |
| 三角形扇 | 直观，适合纯几何管线 | 需要更多顶点；边缘是多边形近似 |
| 矩形 + 片元距离判断 | 顶点少；边缘可抗锯齿；半径容易调整 | 依赖片元着色器；圆外片元仍会被光栅化 |

当前方案适合绘制 UI 圆形、按钮、指示器和简单 2D 图形。如果需要真正的 3D 圆盘、复杂遮挡或几何级别的边界，则可以改用三角形扇、网格细分或更复杂的几何方案。

