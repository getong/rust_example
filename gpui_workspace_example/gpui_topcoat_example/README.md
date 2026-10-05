# Topcoat × GPUI-kit 多路由协作工作台

网页和桌面通过 HTTP 操作同一份服务端内存数据。桌面使用 `gpui-router` + `TabBar`，路由决定当前 Tab，页面 Entity 缓存保留未提交的输入。每个页面两秒同步一次，网络请求在后台运行；失败保留旧值并提示错误，修改操作不会自动重试。

## 页面与复用来源

| 桌面路由 / Tab | Topcoat 网页 | 功能 | 复用来源 |
| --- | --- | --- | --- |
| `/counter` 计数器 | `/` | 增加、重置、共享状态 | 原协作示例 |
| `/todos` 待办事项 | `/todos` | 新建、完成/恢复、删除 | `axum_todo_utoipa_swagger_ui_example/src/todo.rs` |
| `/echo` JSON 回显 | `/echo` | 提交任意 JSON、查看两端共享历史 | `axum_post_echo_json_example/src/main.rs` |
| `/profile` 表单提交 | `/profile` | 姓名/年龄提交、校验和共享历史 | `axum_form_request_example/src/main.rs` |

路由与 Tab 实现改编自本工作区 `entity_view_example/src/tabbed_panel.rs` 和 `panel_tab.rs`：`Routes` 渲染缓存的 Entity，`RouterState` 变化同步选中项，点击 Tab 调用 `use_navigate`。不是只修改选中索引的静态页面。

Axum 示例的业务逻辑改编为 Topcoat 路由、JSON/Form 提取器和共享应用状态；TODO 不包含原示例的 Swagger/OpenAPI 和 API key 功能。JSON 回显扩展为保存最近 20 条回显，方便跨端观察。

## 启动

在 `axum_workspace_example` 终端执行：

```sh
cargo run -p topcoat_gpui_example
```

在 `gpui_workspace_example` 终端执行：

```sh
cargo run -p gpui_topcoat_example
```

浏览器访问 <http://127.0.0.1:3000>，使用顶部导航切换网页。在任一端添加待办、提交 JSON 或表单，另一端约两秒内显示结果。默认只监听本机，服务重启后数据清空。

自定义服务地址：

```sh
# Axum 工作区
HOST=127.0.0.1 PORT=3010 cargo run -p topcoat_gpui_example
# GPUI 工作区
TOPCOAT_URL=http://127.0.0.1:3010 cargo run -p gpui_topcoat_example
```

`TOPCOAT_URL` 使用服务根地址，不附加 `/api`。相邻工作区布局需保留：Topcoat 通过路径依赖引用 `gpui_topcoat_example/protocol`，共享协议不依赖 UI 框架。

## API

| 请求 | 请求体 | 返回 |
| --- | --- | --- |
| `GET /api/counter` | 无 | `{value, revision}` |
| `POST /api/counter` | `{"action":"increment"}` / `{"action":"reset"}` | 更新后的计数器 |
| `GET /api/demos` | 无 | `{todos, echoes, profiles}` |
| `POST /api/todos` | `{"action":"create","title":"任务"}` | 更新后的完整 DemoSnapshot |
| `POST /api/todos` | `{"action":"set_done","id":1,"done":true}` | 同上 |
| `POST /api/todos` | `{"action":"delete","id":1}` | 同上 |
| `POST /api/echo` | 任意合法 JSON | 同上，`echoes[0]` 是此次回显 |
| `POST /api/profile` | URL 编码表单 `username=Alice&age=28` | 同上 |

待办最多 200 条、标题 1–120 个字符；表单姓名 1–80 个字符、年龄 0–150；JSON 最大 64 KiB；回显和表单各保留最近 20 条。无效输入返回 400，不存在的待办返回 404。服务端互斥锁保证并发修改一致。此示例使用内存存储，没有认证与持久化。

## 验证

```sh
# Axum 工作区
cargo test -p topcoat_gpui_example

# GPUI 工作区：实际点击 Tab，验证路由与草稿保留
cargo test -p gpui_topcoat_example --bin gpui_topcoat_example

# GPUI 工作区：构建并进行跨端 HTTP 联调
cargo build --manifest-path ../axum_workspace_example/Cargo.toml -p topcoat_gpui_example
cargo build -p gpui_topcoat_example --no-default-features --bin topcoat-smoke
python3 gpui_topcoat_example/scripts/smoke.py
```

联调脚本启动临时端口服务，模拟浏览器发送 JSON 和表单，然后由桌面实际使用的 Rust HTTP 客户端读取并修改，再从网页接口确认结果；同时检查四个网页、脚本资源和断线错误。结束后自动停止测试服务，不影响已有实例。

人工验收：两端分别增加待办、切换完成状态、删除；分别提交 JSON 和表单；桌面输入草稿后切换 Tab 再返回；停止/重启服务，检查断线提示和自动恢复。

## 独立配色实验室

新增桌面路由 `/studio` 和「配色实验室」Tab，默认进入该页。配套网页也是 `/studio`；双端仅通过新接口 `GET /api/studio`、`POST /api/studio` 共享配色，不复用原有业务接口。

在网页项目先 `npm ci`，再 `cargo run -p topcoat_gpui_example`；Cargo 自动编译 TypeScript，生成的 JS 只存放在构建目录。桌面运行 `cargo run -p gpui_topcoat_example`。

网页用 TypeScript 实时预览主题和强度，点击「发布到两端」后桌面约两秒同步。桌面支持三个主题、强度 ±10、本地预览、发布、读取最新发布及打开对应网页。本地草稿不会被轮询覆盖，失败保留上次数据并提示错误。原生桌面仍由 Rust 渲染。

共享协议新增 `StudioTheme`、`StudioSnapshot`、`StudioCommand`；`studio_request` 是桌面和独立 `studio-smoke` 联调程序共用的客户端。

```sh
cargo test -p gpui_topcoat_example --bin gpui_topcoat_example
cargo build -p gpui_topcoat_example --bins
# 在网页项目目录：
npm run test:e2e
```

E2E 先由真实 TypeScript 网页发布落日橙/35，再由 `studio-smoke` 确认数据并发布森林绿/80，最后验证网页自动更新。测试不调用旧业务 API。
