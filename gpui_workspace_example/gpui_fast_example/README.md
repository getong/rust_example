# GPUI Fast Example

参考 `/Users/gerald/test/rust/gpui-fast` 的 `hello_world` 示例初始化的桌面应用。
本项目已属于上层 Cargo workspace，依赖从 `longbridge/gpui-fast` Git 仓库获取。
依赖版本由上层 `Cargo.lock` 锁定；gpui-fast 要求 `cocoa 0.26.0`，
因此初始化时已同步调整 workspace 锁文件中的版本。

## 运行

在当前目录执行：

```sh
cargo run -p gpui_fast_example
```

需要支持 Rust 2024 edition 的工具链；参考仓库使用 Rust 1.98.1。
macOS 构建需要 Xcode 及其 Metal 编译工具，首次构建需要下载并编译依赖。

窗口包含计数器：点击 **Increment** 加一，点击 **Reset** 清零。
关闭窗口或按 `Cmd+Q`（macOS）/ `Ctrl+Q`（其他平台）退出。

## 代码入口

- `Cargo.toml`：`gpui` 与 `gpui_platform` 使用同一 Git 来源。
- `src/main.rs`：平台初始化、窗口配置、计数器视图与点击事件。

修改状态后调用 `cx.notify()`，让 GPUI Fast 更新保留的视图。
可以关闭 Retained Mode 对比运行行为：

```sh
GPUI_VIEW_RETENTION=0 cargo run -p gpui_fast_example
```

## 检查

```sh
cargo fmt -p gpui_fast_example -- --check
cargo check -p gpui_fast_example
```
