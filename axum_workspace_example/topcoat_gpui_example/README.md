# Topcoat 多页面协作服务

配合相邻 `gpui_workspace_example/gpui_topcoat_example` 桌面项目使用。

```sh
# Axum 工作区
cargo run -p topcoat_gpui_example
```

访问 <http://127.0.0.1:3000>，顶部导航提供四个页面：

- `/`：共享计数器。
- `/todos`：待办新建、完成/恢复、删除。
- `/echo`：JSON 回显与共享历史。
- `/profile`：表单提交与共享历史。

桌面工作区运行 `cargo run -p gpui_topcoat_example`，四个路由 Tab 与网页通过 HTTP 共享状态，每两秒同步。`HOST` / `PORT` 可覆盖默认 `127.0.0.1:3000`；桌面通过 `TOPCOAT_URL` 配置对应地址。

`src/demos.rs` 改编自同一工作区的三个例子：

- `axum_todo_utoipa_swagger_ui_example/src/todo.rs` 的内存 TODO 操作；不包含 Swagger 与认证功能。
- `axum_post_echo_json_example/src/main.rs` 的 JSON 提取/回显，扩展为共享历史。
- `axum_form_request_example/src/main.rs` 的姓名年龄表单提取，增加校验与共享历史。

处理器使用 Topcoat 的 `Json` / `Form`，数据类型来自共享 `topcoat_gpui_protocol`。服务端内存状态重启后清空。

`cargo test -p topcoat_gpui_example` 检查计数器、并发修改、TODO 增删改、非法输入、历史上限和页面路由。

完整 API、双端启动与联调方式见 [桌面项目说明](../../gpui_workspace_example/gpui_topcoat_example/README.md)。
