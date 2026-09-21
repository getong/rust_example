# GPUI Entity 设计模式

根据 `design.org` 实现四种模式，并保留直接组合的基础示例。

| 模块 | 职责 | 验证方式 |
| --- | --- | --- |
| `src/composition.rs` | 基础 Counter，由父组件直接持有 | 点击 +1，仅当前计数变化 |
| `src/events.rs` | `EventEmitter` + `cx.emit` / `cx.subscribe` | 点击子组件，父组件收到的计数同步变化 |
| `src/global_state.rs` | `Global` + `update_global` / `observe_global` | 点击 Shared +1，两个独立读者同步变化 |
| `src/slots.rs` | 接受 `Vec<AnyView>` 的通用容器 | Profile 与 Counter 同时呈现，Counter 可独立操作 |
| `src/observe.rs` | `cx.observe` 监听源实体的 `cx.notify` | 点击 Source +1，派生视图显示其两倍 |
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
