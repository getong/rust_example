# winit + wgpu 多窗口实时渲染示例

这个示例说明桌面 Rust 程序中 `winit` 与 `wgpu` 的常见组合方式：

- `winit` 创建窗口、接收输入和驱动事件循环。
- `wgpu::Instance` 连接图形后端；`Adapter` 选择 GPU；`Device`/`Queue` 是共享的 GPU 资源。
- 每个窗口都有自己的 `wgpu::Surface` 和 `SurfaceConfiguration`。
- 每帧从每个 surface 获取 `SurfaceTexture`，创建 render pass 清屏，提交 command buffer 后调用 `Queue::present()`。

## 运行

```text
cargo run
```

程序启动后会创建一个窗口。按 `N` 创建更多窗口；每个窗口都会独立实时刷新，并且可以分别调整大小或关闭。关闭最后一个窗口会退出程序。

## 关键设计

`Renderer` 只保存一份 `Device` 和 `Queue`，这避免了每个窗口重复初始化 GPU。`WindowState` 保存窗口自己的 `Arc<Window>`、`Surface` 和配置，因此窗口 resize、surface 丢失或过期时只影响当前窗口。

`about_to_wait` 中为所有窗口调用 `request_redraw`，`WindowEvent::RedrawRequested` 中完成实际绘制。将控制流设为 `ControlFlow::Poll` 可以让动画持续运行，而不是只在输入事件发生时刷新。

窗口内容现在是一个由 WGSL shader 生成的 3D 场景：中心立方体周围有两个缩放后的立方体进行轨道运动，包含透视投影、深度测试、面法线、漫反射、镜面高光和边缘光：

- `winit` 的 `about_to_wait` 持续请求重绘。
- `WindowEvent::RedrawRequested` 获取当前窗口的 surface texture。
- `wgpu::Queue::write_buffer` 更新时间和宽高比 uniform。
- render pipeline 执行 vertex shader 和 fragment shader，绘制 36 个立方体顶点、3 个实例。
- command buffer 提交后由 `Queue::present` 显示到当前窗口。

立体效果由三部分组成：vertex shader 根据顶点索引生成立方体六个面；深度缓冲确保前后遮挡关系正确；fragment shader 根据法线计算漫反射、镜面高光和 rim light。

因此每个窗口都不是静态空白画布，而是独立参与同一 GPU 上下文的实时渲染。按 `N` 创建多个窗口，可以观察它们同时动画和分别响应 resize。
