# 远行 · GPUI Kit

使用 Rust 和 `gpui-kit 0.6.6` 实现的原生旅行应用界面，参考 MAI-UI 8B 的 `design-output/result.html`、`detailed-v3/preview.png` 和 `interactive/result.html`。

```sh
cargo run -p mai_ui_demo
```

默认窗口为 480 × 900，内容可滚动，顶部栏和底部导航保持可见；放大窗口后保留居中的窄屏布局。

- 首页：大理主题、原生 Canvas 山水插画、行程与预算概览。
- 每日安排：Day 1–4 切换日期、景点、时间和路线提示。
- 收藏：点击沙溪古镇爱心添加或取消，收藏页面与个人页计数同步。
- 准备清单：点击条目切换完成状态，进度条即时更新。
- 导航：首页、行程、收藏、我的；右上角铃铛显示/隐藏出发提醒。

这是本地 UI 演示：金额、天气、出发倒计时沿用设计稿的示例数据，Day 2–4 为补充演示内容；无网络请求，交互状态只保留在本次运行中。

源码按 Tab 页面和共享功能组织：

```text
src/
├── main.rs                  # 窗口初始化
└── travel/
    ├── mod.rs               # 模块入口，仅导出 TravelApp
    ├── app.rs               # 根视图、Tab 分发、滚动容器
    ├── state.rs             # 当前 Tab、日期、收藏与准备状态
    ├── data.rs              # 每日行程和路线的演示数据
    ├── theme.rs             # 共享配色
    ├── tabs/
    │   ├── home.rs          # 首页
    │   ├── trips.rs         # 行程页
    │   ├── saved.rs         # 收藏页
    │   └── profile.rs       # 我的页
    └── components/
        ├── common.rs        # 布局、文字、卡片基础组件
        ├── icon.rs          # SVG 图标
        ├── landscape.rs     # Canvas 山水插画
        ├── header.rs        # 顶部栏
        ├── navigation.rs    # 底部导航
        ├── hero.rs          # 旅行主题与出发信息
        ├── overview.rs      # 预算、预订、天气概览
        ├── schedule.rs      # 每日日程切换和列表
        ├── recommendation.rs # 推荐地点与收藏操作
        └── checklist.rs     # 准备清单与进度
```

`TravelApp` 持有唯一的 `TravelState`。各 Tab 组合共享组件，组件事件通过根视图的 `Context<TravelApp>` 更新状态并通知重绘，因此切换页面不会重置收藏、选中日期或清单。模块内部 API 的可见范围限制在 `travel` 内。

验证：`cargo check -p mai_ui_demo --offline`、`cargo clippy -p mai_ui_demo --offline -- -D warnings`、`cargo fmt -p mai_ui_demo --check`。
