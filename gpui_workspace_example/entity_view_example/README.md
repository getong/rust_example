# 同一 Panel 内的多 Tab 状态同步 Demo

仅依赖 `gpui-kit`，使用一个原生窗口、一个 `Root` 和一个 `TabbedPanel`。
面板默认包含两个计数标签页、一个 `Toast` 标签页和一个 `Scrollbar` 标签页。
计数标签拥有独立视图，共享同一个计数模型与步长配置。

`Scrollbar` 标签包含 80 行内容和常驻的垂直滚动条，支持滚轮、触控板和拖动滚动条。
点击 `Back to top` 回到顶部；切换标签保留各自的滚动位置。
点击任意行会选中该行并显示 Toast，选中状态也会在切换标签后保留。
`Remember position` 记录当前位置，`Restore position` 返回记录的位置，方便回到顶部后继续浏览。
这些状态独立保存在每个打开的标签中；关闭标签或退出应用后不保留。
点击 `New scroll tab` 新增独立的滚动标签，也可以关闭后重新添加。

点击 `Toast` 标签，再点击 `Success`、`Info`、`Warning` 或 `Error` 显示对应弹窗。
弹窗在 5 秒后自动消失，悬停暂停计时，也可点击常驻的 `Close` 按钮立即关闭。
点击 `New toast tab` 可新增并选中 Toast 标签。
通知属于当前窗口，切换或关闭标签不会立即清除已有弹窗。
实现参考 gpui-kit 的 `notification_story.rs`，通过 `WindowExt::push_notification` 发送，
在面板顶层挂载 `Root::render_notification_layer`（Notification 内部使用 Base Toast）。

## 运行

```sh
cargo run -p entity_view_example
```

1. 在 Tab 1 点击 Counter A 的 +1，然后切换 Tab 2：A 和汇总已同步。
2. 在 Tab 2 切换步长，再返回 Tab 1：递增按钮显示 +5，已有计数不变。
3. 点击 `Only this tab`：只增加当前标签的局部次数；切换回来仍保留。
4. 点击 `New tab`：在同一面板新增并选中新标签，立即显示共享计数。
5. 点击 `Close current tab`：关闭当前标签；其余标签继续共享状态。
6. 任意标签重置：所有标签的共享计数归零，步长与局部次数不变。
7. 可以关闭全部标签（包括 Toast 和 Scrollbar），再点 `New tab`，共享计数仍保留。

## 实现

- `AppSettings: Global`：应用级步长配置。
- `AppServices: Global`：持有唯一的 `Entity<CounterState>`。
- `TabbedPanel`：持有标签实体列表、选中索引和下一个标签编号。
- `CounterTab`：每个标签的局部次数、计数视图和汇总视图。
- `CounterState`：共享的 A、B 计数；汇总直接派生。

```text
Tab 1 按钮 → 共享 Entity::update → 模型 notify
                                  ↓
                  所有标签的 observe → 各视图 notify
                                  ↓
                    当前标签刷新；切换标签读取最新值
```

`Entity::clone` 只克隆句柄，不复制数据。切换标签不重建视图，隐藏标签仍持有订阅。
各标签保存自己的 Subscription；关闭标签后释放视图并取消订阅。
Global 和面板继续持有模型，因此关闭全部标签也不会清空共享数据。
状态只保存在当前进程内存中，重启应用恢复默认值。

详细说明：[Tab 通讯与演示步骤](docs/tab-panel.md)。
基础机制：[GPUI 的订阅、通知与状态更新](docs/state-subscriptions.md)。

## 验证

```sh
cargo test -p entity_view_example
cargo clippy -p entity_view_example --all-targets --no-deps -- -D warnings
```

测试覆盖模型通知、Global 通知、生命周期，以及同一测试窗口内点击标签、跨标签修改、
局部状态保留、新增与关闭标签、空面板重新添加标签。
