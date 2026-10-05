# Topcoat 多页面协作服务

配合相邻 `gpui_workspace_example/gpui_topcoat_example` 桌面项目使用。

```sh
# Axum 工作区
cargo run -p topcoat_gpui_example
```

访问 <http://127.0.0.1:3000>，顶部导航提供五个页面：

- `/`：共享计数器。
- `/todos`：待办新建、完成/恢复、删除。
- `/echo`：JSON 回显与共享历史。
- `/profile`：表单提交与共享历史。
- `/studio`：独立配色实验室，实时预览并同步到桌面。

桌面工作区运行 `cargo run -p gpui_topcoat_example`，五个路由 Tab 与网页通过 HTTP 共享状态，每两秒同步。`HOST` / `PORT` 可覆盖默认 `127.0.0.1:3000`；桌面通过 `TOPCOAT_URL` 配置对应地址。

`src/demos.rs` 改编自同一工作区的三个例子：

- `axum_todo_utoipa_swagger_ui_example/src/todo.rs` 的内存 TODO 操作；不包含 Swagger 与认证功能。
- `axum_post_echo_json_example/src/main.rs` 的 JSON 提取/回显，扩展为共享历史。
- `axum_form_request_example/src/main.rs` 的姓名年龄表单提取，增加校验与共享历史。

处理器使用 Topcoat 的 `Json` / `Form`，数据类型来自共享 `topcoat_gpui_protocol`。服务端内存状态重启后清空。

`cargo test -p topcoat_gpui_example` 检查计数器、并发修改、TODO 增删改、非法输入、历史上限和页面路由。

完整 API、双端启动与联调方式见 [桌面项目说明](../../gpui_workspace_example/gpui_topcoat_example/README.md)。

## TypeScript 配色实验室

所有手写前端、构建、测试和配置文件均为 `.ts`，不保存 `.js` / `.mjs` 源文件。首次构建先在本目录执行 `npm ci`，之后运行 `npm start` 或 `cargo run -p topcoat_gpui_example`。Cargo 的 `build.rs` 自动执行严格类型检查和 esbuild，将浏览器可执行的 JavaScript 生成到 Cargo `OUT_DIR` 并嵌入二进制。运行服务无需 Node，编译服务需要 Node/npm 和已安装的依赖；修改 TS 后重新编译、启动服务。

网页路由 `/studio`，独立 API `GET /api/studio`、`POST /api/studio`，资源路由 `/assets/studio`。该页面不会访问计数器、待办、回显或表单 API。状态独立保存在服务端内存中，重启后恢复默认。

```json
{"action":"apply","theme":"sunset","intensity":35}
```

返回 `{theme, intensity, revision}`；主题为 `ocean | sunset | forest`，强度为 0–100 整数。每次发布增加版本；未知主题、无效强度、额外字段会被拒绝。

1. 启动网页服务和桌面，桌面默认进入新 Tab「配色实验室」。
2. 点击「打开配色网页」，或访问 http://127.0.0.1:3000/studio。
3. 网页选择主题、拖动强度滑块，TypeScript 立即更新预览；此时尚未修改共享状态。
4. 点击「发布到两端」，桌面约两秒后显示相同配色；桌面也可调整并发布，网页会自动同步。
5. 未发布的本地草稿不会被轮询覆盖；「使用最新发布」放弃草稿并读取最新配色。

失败时保留上次配色，发布不会自动重试。GPUI 保持 Rust 原生渲染，网页使用 TypeScript，通过独立协议协作。

```sh
npm test                         # 严格检查 + 协议边界测试
cargo test -p topcoat_gpui_example
cargo build --manifest-path ../../gpui_workspace_example/Cargo.toml -p gpui_topcoat_example --bins
npx playwright install chromium
npm run test:e2e                  # 独立 3198 端口，结束后自动停止
```

E2E 运行真实浏览器中的编译产物，检查预览不发布、发布后原生客户端读取、原生客户端修改后网页同步、草稿保留、无效响应恢复，并断言新页面只访问 `/api/studio`。桌面点击测试在 GPUI 工作区运行 `cargo test -p gpui_topcoat_example --bin gpui_topcoat_example`。
