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

窗口内容现在包含一个由 WGSL shader 生成的旋转彩色方块和三角形：

- `winit` 的 `about_to_wait` 持续请求重绘。
- `WindowEvent::RedrawRequested` 获取当前窗口的 surface texture。
- `wgpu::Queue::write_buffer` 更新时间和宽高比 uniform。
- render pipeline 执行 vertex shader 和 fragment shader，绘制实际几何图形。
- command buffer 提交后由 `Queue::present` 显示到当前窗口。

因此每个窗口都不是静态空白画布，而是独立参与同一 GPU 上下文的实时渲染。按 `N` 创建多个窗口，可以观察它们同时动画和分别响应 resize。
