# 同一个 Panel 中的 Tab 通讯与状态同步

## 结构

```text
App
├── AppSettings (Global)：step
├── AppServices (Global)：Entity<CounterState>（唯一模型）
└── 原生 Window / Root / TabbedPanel
    ├── TabBar：选择当前标签
    ├── Tab 1 / CounterTab：独立局部次数与子视图
    ├── Tab 2 / CounterTab：独立局部次数与子视图
    └── 后续新增标签……（全部持有同一模型句柄）
```

应用只调用一次 `open_window`。标签由 Kit 的 `TabBar` / `Tab` 显示，
`TabbedPanel` 持有 `Vec<Entity<CounterTab>>`，仅将选中标签加入内容区域。
这是一组同面板标签页，不包含浮动窗口、拖拽拆分或 Dock 布局。

模型在 `main` 初始化时创建一次。标签构造函数接收模型句柄，不创建自己的计数模型。
标签编号单调递增，与列表索引分开；关闭标签后不复用旧标签的局部状态。

## 状态更新链

1. Tab 1 的按钮调用共享 `CounterState` 的业务方法。
2. 模型同步修改数据并 `cx.notify()`。
3. 各标签子视图的 `observe` 回调收到通知，再通知自身刷新。
4. 当前标签重新渲染；切换至隐藏标签时，从同一模型读取最新值。

隐藏只表示没有把标签加入当前内容区域，不代表销毁 Entity；因此订阅持续有效。
标签切换只修改 `active_tab`，局部计数不会因切换归零。
无需在切换时从 Tab 1 复制数据到 Tab 2，也不需要标签互相引用。

步长走 `update_global<AppSettings>` → `observe_global<AppSettings>` → 各视图刷新。
Global 中保存的 Entity 发出 notify，不会自动变成 AppServices 的 Global 通知；
计数消费者订阅的是模型本身。

局部按钮只更新对应 `CounterTab::local_clicks`，不修改共享模型。
模型通知是读取最新状态的信号，不能用通知次数当业务操作次数；本例不需要一次性事件总线。

## 演示步骤

```sh
cargo run -p entity_view_example
```

| 操作 | 预期结果 |
| --- | --- |
| Tab 1 中 A +1，然后切换 Tab 2 | Tab 2 显示 A = 1、Total = 1 |
| Tab 2 切换步长，再 B +5 | 共享数据 A = 1、B = 5、Total = 6 |
| 返回 Tab 1 | 同样显示 Total = 6，按钮显示 +5 |
| Tab 1 点击 Only this tab，再切换 Tab 2 | Tab 2 的局部次数仍为 0 |
| 返回 Tab 1 | 局部次数仍为 1，未被重建 |
| New tab | 新 Tab 3 显示已有共享计数，局部次数为 0 |
| Close current tab | 关闭 Tab 3，选择相邻的有效标签，共享计数保持 |
| Reset both counters | 所有标签 A、B 归零；步长和各自局部次数不变 |
| 修改计数，关闭所有标签，再 New tab | 新标签读取保留的共享计数与配置 |

## 生命周期与边界

每个标签保存自己的 Subscription。关闭标签从列表移除强句柄，GPUI 在清理周期释放视图与订阅。
切换到其他标签不会释放原标签。面板和 AppServices 持有模型，因此空面板仍保留共享状态。
应用退出后内存状态消失，本例不做持久化。

关闭当前标签后选中同位置的下一个标签；若已是末尾则选中前一个。空面板禁用关闭按钮。
新标签始终被选中。标签过多时使用 Kit TabBar 的滚动行为。

## 自动验证

`src/tests.rs` 在单个 GPUI 测试窗口中点击真实标签和按钮，验证隐藏标签接收通知、
跨标签配置与计数同步、切换后局部次数保留、新增标签读取最新值、关闭标签释放实体、
关闭全部标签后重新创建，以及重置行为。
其余测试覆盖基础模型通知和订阅生命周期。测试窗口不等同于原生桌面外观验证。
