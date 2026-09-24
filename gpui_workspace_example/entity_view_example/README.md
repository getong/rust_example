# 同一 Panel 内的多 Tab 状态同步 Demo

使用 `gpui-kit` 与 `gpui-router`，一个原生窗口、一个 `Root` 和一个 `TabbedPanel`。
`gpui-router` 的 `Routes` / `Route` 匹配并渲染已打开的标签视图。
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
再次点击入口会返回已有目录；关闭目录后可以重新打开。

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
- `TabbedPanel`（`src/tabbed_panel.rs`）：管理标签的打开、复用、关闭；观察 `RouterState`，从当前路径派生选中标签。
- `TabId` / `PanelTab`（`src/panel_tab.rs`）：类型化标签标识与统一页面记录；创建时保存路径、标题和 `AnyView`，无需在渲染中读取页面状态来拼接路径。
- `Routes` / `Route`：为已打开标签声明具体路径，渲染已有 Entity，切换时不重建页面。
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

## gpui-form 表单生成示例

在 **All components → gpui-form** 打开演示页（`/component/gpui-form`）。
源码：`src/tabs/gpui_form_tab.rs`；详细说明：[gpui-form 的功能与作用](docs/gpui-form.md)。

点击「回填示例」，修改名称与端口后提交；输入 `abc`、`0` 或 `70000` 可观察转换错误。
页面实时显示编辑草稿，支持重置；提交仅在本地构造业务模型，不发送网络请求。

## gpui-toolkit-gpui-util 工具案例

在 **All components → gpui-toolkit-gpui-util** 打开交互页（`/component/gpui-util`），
演示 ArcCow 借用与共享、Result/Future 错误处理、defer 清理、measure 计时和辅助工具。
点击页面中的「运行案例」查看结果；defer 支持「运行并取消清理」进行对照。
功能、适用场景和限制见 [gpui-util 说明](docs/gpui-util.md)。Rust 导入名是 `gpui_util`。

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

业务代码通过面板导航，只允许访问已打开的标签；失败返回 `false`，保持当前页面：

```rust
panel.navigate("/counter/2", cx);
```

也可以调用 `gpui_router::use_navigate(cx)("/counter/2".into())`；面板通过
`observe_global::<RouterState>` 同步 TabBar 和内容。底层 hook 不检查标签是否存在，
访问不存在的路径会显示空页面。应用使用一个全局路由状态，适用于当前单窗口、单面板结构，
不提供多面板独立导航。

新增标签时用 `PanelTab::new(id, label, entity)` 保存页面，`PanelTab::route()` 统一转换成
`gpui-router::Route`。`Routes` 的声明只需一行：

```rust
Routes::new().children(self.tabs.iter().map(PanelTab::route))
```

`TabId` 决定稳定路径，`AnyView` 保留原始 Entity 的所有权；增加页面类型无需再维护
路径、标题、视图三组 `match`。组件与百度页按逻辑标识复用，多实例页按 Entity ID 区分。
路径变化由同一个观察器触发重绘并滚动到选中标签，目录也调用 `navigate`；
观察器忽略 `Routes` 在渲染时写入的匹配元数据，避免反复重绘。
标签列表增删单独发出面板通知，确保目录及时更新。关闭时移除页面记录及其路由。
所有标签关闭后导航到 `/`，可重新新增标签，共享模型保持不变。
状态栏的 ID 是标签路径末段。路径只在本次运行有效，不是 OS 深链接或持久化地址。

依赖启用 `gpui-router` 的 `gpui-pre` 后端并关闭默认后端，与 `gpui-kit` 共用 GPUI 类型。
本项目不再直接依赖路径匹配库，也不维护模板注册、模板错误或导航栈适配层。

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
