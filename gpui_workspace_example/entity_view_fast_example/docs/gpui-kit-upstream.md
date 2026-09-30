# GPUI Kit 与 GPUI Fast

此应用使用 GPUI Kit 组件与 GPUI Fast 渲染后端。

## 窗口与状态

窗口由 `gpui_platform::application()` 创建，`Root` 挂载组件插件与浮层。
共享数据由 `Entity<CounterState>` 管理，视图通过 `observe` 订阅变更。
每个标签保存自己的视图实体和局部状态，切换标签时不会重建。

## 对话框

通过窗口扩展打开对话框。Root 插件负责渲染对话框、Sheet 和通知，
页面无需手动重复挂载浮层。关闭对话框后可继续操作原来的标签。

## 项目来源

- GPUI Kit: https://github.com/longbridge/gpui-kit
- GPUI Fast: https://github.com/longbridge/gpui-fast

具体依赖版本见本项目 Cargo.lock；运行与验证方式见 README.md。
