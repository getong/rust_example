# 原生 Entity + Global 状态管理

仅依赖 `gpui-kit`，使用它导出的 GPUI 原生 API。

## 状态划分

- **领域 Model**：`Entity<CounterState>` 存放两个计数值和操作说明，不实现 Render。
- **视图 Entity**：两个 CounterPanel、一个 SummaryPanel 和父视图 CounterApp 实现 Render。
- **全局配置**：`AppSettings: Global` 存放递增步长（1 或 5），在创建窗口前通过 set_global 初始化。

当前版本用 `cx.new()` 统一创建 Model 和 View；不要使用旧教程的 `new_model()` / `new_view()`。
Global 按类型存放在 App 内，并非进程级 static。

## Entity 通信

```text
计数器 / 汇总按钮 -> Entity::update -> CounterState 修改数据并 cx.notify()
                                            │
                         各视图 cx.observe 回调 -> 视图 cx.notify() -> 重绘

切换步长按钮 -> cx.update_global<AppSettings>()
                         │
             cx.observe_global<AppSettings>() -> 父视图和两个计数器刷新
```

`Entity::update` 同步修改状态；观察回调与渲染由 GPUI 调度。
视图通过 `Entity::read` 读取唯一 Model；总数直接派生，不复制计数状态。
持有句柄或调用 read 本身不等于建立订阅，本例通过 observe 显式建立刷新关系。
观察整个 Model，因此任意计数变化都会通知三个子视图，不是字段级追踪。
Summary 不依赖配置，所以无需 observe_global；切换配置不会修改领域状态。

`Entity::clone` 只复制强句柄。三个视图共同持有 Model，最后一个强句柄释放后 Model 才可释放。
视图保存 Subscription，其销毁时自动取消订阅；Model 不反向强引用视图。
在构造函数中创建实体、订阅，不在 render 中反复创建。

- `src/state.rs`：领域数据、更新方法与全局配置。
- `src/main.rs`：原生视图、观察关系、按钮交互和初始化，包含中文说明。
- `src/tests.rs`：验证共享更新、重置、Global 通知和生命周期。

## 运行

```sh
cargo run -p entity_view_example
cargo test -p entity_view_example
cargo clippy -p entity_view_example --all-targets --no-deps -- -D warnings
```

分别点击两个计数器，汇总会同步更新；切换步长后两个按钮同步变为 +5 / +1；
重置同时归零两个计数器，但保留全局步长配置。
