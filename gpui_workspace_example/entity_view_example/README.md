# 同一 Panel 内的多 Tab 状态同步 Demo

使用 `gpui-kit` 与 `matchit`，一个原生窗口、一个 `Root` 和一个 `TabbedPanel`。
`matchit::Router` 将路径解析为标签视图，Kit 的 `NavStack` 承载当前页面。
面板默认包含两个计数标签页、一个 `Toast` 标签页和一个 `Scrollbar` 标签页。
计数标签拥有独立视图，共享同一个计数模型与步长配置。

`Scrollbar` 标签包含 80 行内容和常驻的垂直滚动条，支持滚轮、触控板和拖动滚动条。
点击 `Back to top` 回到顶部；切换标签保留各自的滚动位置。
点击任意行会选中该行并显示 Toast，选中状态也会在切换标签后保留。
`Remember position` 记录当前位置，`Restore position` 返回记录的位置，方便回到顶部后继续浏览。
这些状态独立保存在每个打开的标签中；关闭标签或退出应用后不保留。
点击 `New scroll tab` 新增独立的滚动标签，也可以关闭后重新添加。

点击工具栏 `Tab directory` 打开标签目录页。目录使用常驻垂直滚动条，列出当前面板
所有可跳转的标签（包含目录自身），每项显示名称和路径；点击条目即可切换标签。
新增或关闭标签时列表自动增减，切换页面会保留目录的滚动位置。
再次点击入口会返回已有目录；关闭目录后可以重新打开，各面板的目录互不影响。

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
- `TabbedPanel`：持有标签实体列表和面板独立的 `Entity<TabRouter>`；选中标签从路由派生。
- `TabRouter`：`matchit` 匹配动态路径模板，按具体路径保存页面实例，`NavStackState` 切换视图。
- `CounterTab`：每个标签的局部次数、计数视图和汇总视图。
- `TabDirectory`：观察面板变化，从实时标签列表派生可点击的滚动目录；弱引用面板以避免循环持有。
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
局部状态保留、新增与关闭标签、空面板重新添加标签，以及目录增删同步、点击跳转、滚动位置与释放。

## 路由跳转

点击标签、创建标签、关闭当前标签都会更新路由。计数标签使用 `/counter/1`、
`/counter/2` 等路径；Toast 和 Scrollbar 使用 `/toast/<entity-id>`、
`/scrollbar/<entity-id>`，允许多个同类标签并存。界面显示当前路径。

业务代码也可以直接调用面板的 router，观察者会同步更新 TabBar 和页面：

```rust
panel.router.update(cx, |router, cx| {
  router.navigate("/counter/2", cx)
})?;
```

路由只访问已打开的标签；`register_route`、`register`、`navigate` 统一返回自定义
`RouterError`，公共错误类型不依赖 `matchit`。导航失败时当前页面和参数不变：

- `UnknownPath`：路径未匹配任何已注册模板。
- `ClosedPath`：路径匹配模板，但对应页面尚未注册或已经关闭。

关闭标签注销页面实例并释放对应视图；关闭全部标签清空导航栈。各面板的路由独立，
共享计数仍通过同一个模型同步。路径只在本次运行有效，不是 OS 深链接或持久化地址。

### 动态路由与动态页面

面板注册 `/counter/{id}`、`/toast/{id}`、`/scrollbar/{id}`、`/tabs/{id}` 四个 matchit 模板。
点击新增按钮时，将新 Entity 注册到具体路径，然后导航；同一模板下的不同 ID
各自持有页面状态。`router.param("id")` 可读取当前路径中的 ID。

路由模板也支持在运行时添加，顺序为定义模板、注册实例、导航：

```rust
router.register_route("/projects/{project}/files/{*file}")?;
router.register("/projects/demo/files/src/main.rs", view)?;
router.navigate("/projects/demo/files/src/main.rs", cx)?;
assert_eq!(router.param("project"), Some("demo"));
assert_eq!(router.param("file"), Some("src/main.rs"));
```

`register_route` 支持静态路径、命名参数和 catch-all；非法或冲突模板返回
`RouterError::InvalidPattern { reason }` 或 `RouterError::ConflictingPattern { with }`。
`reason` 保留底层诊断文本，仅用于展示；`with` 指明冲突的已有模板（重复注册也属于冲突）。
`register` 对未匹配模板的路径返回 `RouterError::UnknownPath`，对重复页面返回
`RouterError::DuplicatePath`，不覆盖原实例。
模板匹配成功也必须存在对应页面才能导航，不会自动创建任意 ID 的页面。
`unregister` 接收具体路径，只释放该实例，模板继续供其他实例和后续新增页面使用。
上述 API 操作路由和页面承载；新增 TabBar 标签仍通过面板的 `open_tab` 同步维护标签列表。

### 库选择（2026-09-20 核对 GitHub）

| 方案 | 当前依赖/适用性 |
| --- | --- |
| [gpui-router](https://github.com/justjavac/gpui-router/blob/main/Cargo.toml) | `gpui 0.2.1`，与 Kit 的 `gpui-pre` 类型不统一 |
| [gpui-navigator](https://github.com/vanyastaff/gpui-navigator/blob/main/Cargo.toml) | `gpui 0.2`，同样需要适配 |
| [gpui-navi](https://github.com/elcoosp/gpui-navi/blob/main/Cargo.toml) | 直接依赖 Zed Git GPUI，未对齐当前 Kit |
| [matchit](https://github.com/ibraheemdev/matchit) + [Kit NavStack](https://github.com/longbridge/gpui-kit/blob/main/crates/base/src/nav_stack.rs) | 本项目采用：路径匹配独立于 UI，页面导航使用 Kit 原生类型 |

这是 `matchit` 路由库加少量应用适配代码，并非现成的 GPUI 声明式路由框架。
不需要引入另一套 GPUI，也无需 vendor/fork。当前平级标签使用 `replace`，不累积浏览历史。

### GPUI 基础教学页

启动时会随画廊打开两个独立标签，关闭后可从 **All components** 重新打开：

- **GPUI 元素**（`/component/gpui-elements`）：介绍文本、`svg/img`、`canvas`、虚拟列表、锚点浮层和自定义视图，附 Zed examples 对应文件与 API 示例。
- **div() 实验室**（`/component/div-lab`）：切换矩形/胶囊/圆形、1×/1.5×/2×，比较只改变盒子尺寸与同步放大文字/间距；点击 div 按钮推进计数与进度，切换 div 开关改变卡片状态。切换标签保留状态，重置恢复初始值。

参考目录：`/Users/gerald/test/rust/zed/crates/gpui/examples`。这些教学页面使用当前项目的 `gpui-kit` API，不依赖该绝对路径加载资源。


GPUI 元素页现有 19 个可切换示例（包含新建独立 Window），源码位于 `src/tabs/gpui_elements_tab/`。
每个源码模块同时用于编译和界面展示，包含 imports 与 `render(window, cx)` 入口；
图片示例使用仓库内置的 `assets/gpui-elements-grid.png`，无需联网。
`Surface` 示例提供 macOS 原生缓冲接入函数，运行画面需由调用方提供 `CVPixelBuffer`；
`Drawable` 示例使用公开的 `AnyElement` 绘制生命周期 API。
