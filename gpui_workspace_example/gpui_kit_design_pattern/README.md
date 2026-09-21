# GPUI Entity 设计模式

根据 `design.org` 实现六种模式，并保留直接组合的基础示例。

| 模块 | 职责 | 验证方式 |
| --- | --- | --- |
| `src/composition.rs` | 基础 Counter，由父组件直接持有 | 点击 +1，仅当前计数变化 |
| `src/events.rs` | `EventEmitter` + `cx.emit` / `cx.subscribe` | 点击子组件，父组件收到的计数同步变化 |
| `src/global_state.rs` | `Global` + `update_global` / `observe_global` | 点击 Shared +1，两个独立读者同步变化 |
| `src/slots.rs` | 接受 `Vec<AnyView>` 的通用容器 | Profile 与 Counter 同时呈现，Counter 可独立操作 |
| `src/observe.rs` | `cx.observe` 监听源实体的 `cx.notify` | 点击 Source +1，派生视图显示其两倍 |
| `src/controller.rs` | 纯数据控制器通过 `subscribe_self` 处理命令 | A/B 两个视图发送通知，共享总数；Reset all 清零 |
| `src/async_bridge.rs` | 后台任务 → 主线程转发信号 → 模型自监听 → UI 观察 | 分别模拟成功和失败，等待一秒看到结果，可重试 |
| `src/app.rs` | 创建各示例，装配页面及 Slot 内容 | 页面可滚动查看所有示例 |
| `src/main.rs` | 初始化组件库、全局状态和窗口 | — |

```sh
cargo run -p gpui_kit_design_pattern
```

文档中的 `cx.publish`、`cx.new_model` 是示意写法；当前依赖使用
`cx.emit`、`cx.new`。事件发布者需实现 `EventEmitter<Event>`。

订阅在构造时注册，由所属实体保存 `Subscription`，实体释放时自动取消。
`notify` 用于状态观察及重绘，`emit` 用于发送业务事件；两者不能互相替代。
全局状态在创建任何读者前注册，修改时使用 `update_global` 通知读者。

## 第 37–115 行的两种模式与当前 API

`gpui-kit 0.6.6` 没有文档中的 `cx.listen()`、`cx.emitter()`。
示例使用真实 API 实现相同的架构意图：

- **集中式控制器**：`AppController` 不实现 `Render`，构造时用
  `cx.subscribe_self()` 注册命令处理。两个发送视图持有同一实体的克隆，
  在 `controller.update` 中调用目标实体的 `cx.emit`；观察者在控制器
  `notify` 后读取总数。此例在装配时共享控制器，无须注册为 `Global`。
- **Async Bridge**：后台执行器模拟下载，仅返回 `LoaderSignal`。
  主线程上的 `cx.spawn` 等待结果，经 `WeakEntity::update` 在模型上下文
  中 `emit`。模型的自监听器更新状态，视图通过 `observe` 刷新。
  成功显示 1024 字节，失败可重试；加载中禁止重复请求。任务句柄由模型
  保存，模型释放时取消等待。这里演示跨任务桥接，未实现跨进程 IPC 或真实网络下载。

`emit` 是实体级事件，并不会向所有实体广播；持有模型引用也不意味着
可以在其他实体的上下文直接向它发事件。`cx.listener()` 则是 UI 回调适配器，
并不是文档所说的信号自监听 API。

```sh
cargo test -p gpui_kit_design_pattern
```

测试覆盖自监听命令处理与重置、后台结果投递、失败后重试、重复请求保护，
以及加载过程中释放模型。
