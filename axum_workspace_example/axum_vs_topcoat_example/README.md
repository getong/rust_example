# axum vs topcoat

同一个最小服务端渲染页面，用两个框架各实现一遍，用来对比 **axum** 和 **topcoat**。

- `src/bin/axum_app.rs` —— [axum](https://github.com/tokio-rs/axum) `0.8.9` 版本（手写 HTML）
- `src/bin/topcoat_app.rs` —— [topcoat](https://github.com/tokio-rs/topcoat) `0.12.1` 版本（`view!` 模板 + `#[component]`）

## 运行

```bash
# 在 workspace 根目录执行

# axum 版，监听 http://127.0.0.1:3000
cargo run -p axum_vs_topcoat_example --bin axum_app

# topcoat 版，监听 http://127.0.0.1:3000（HOST/PORT 环境变量可覆盖）
cargo run -p axum_vs_topcoat_example --bin topcoat_app
```

两个程序都默认占用 3000 端口，同时运行时请给其中一个换端口，例如 topcoat 版：

```bash
PORT=3001 cargo run -p axum_vs_topcoat_example --bin topcoat_app
```

## 代码对比

**axum**（约 15 行有效代码）：

```rust
let app = Router::new().route("/", get(home));
let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
axum::serve(listener, app).await.unwrap();

async fn home() -> Html<&'static str> { Html(PAGE) } // HTML 手写或自行接模板引擎
```

**topcoat**（路由自动发现 + 声明式模板 + 异步组件）：

```rust
topcoat::start(Router::builder().discover().build()).await.unwrap();

#[page("/")]
async fn home(cx: &Cx) -> Result<impl View> {
    Ok(view! { <h1>"Hello"</h1> })
}

#[component]
async fn hello(cx: &Cx, name: &str) -> Result<impl View> {
    Ok(view! { <h1>"Hello, " (name) "!"</h1> })
}
```

（注意：topcoat 0.12.1 要求 `#[page]` / `#[component]` 函数的第一个参数是 `cx: &Cx`。）

## 核心差异

| 维度 | axum | topcoat |
| --- | --- | --- |
| 定位 | 最小化、灵活的 HTTP 框架 / 路由 + 中间件 | 全栈、电池齐全的服务端渲染框架 |
| 成熟度 | 非常成熟、广泛用于生产、API 稳定 | **Early-stage and experimental，预期会有 breaking changes** |
| HTTP 层 | 基于 `tower` / `hyper`，中间件生态最丰富 | 基于 `hyper`（`matchit` 路由），可选 `tower` feature 暴露 `TowerService` |
| 路由 | 手动 `Router::new().route(...)` | 手动 + `#[page]`/`#[component]` 自动发现（`discover`）+ 基于模块结构的 `module_router!` |
| 模板 | 不内置；自选 `askama` / `minijinja` / `handlebars` 等 | 内置类型安全 `view!` 宏，`for` / `if` 等 Rust 控制流直接写在 HTML 里，组件可 async |
| 前端交互 | 自己搭 API + 前端（或 htmx / wasm 等） | 服务端渲染 + 内置轻量客户端响应式：`$(...)` 双端表达式、`signal`、`@click`、`:bind`、`#[shard]` 局部重渲染、`#[procedure]`、无需 wasm/构建步骤 |
| 静态资源 | 自己处理（`tower-http::services::ServeDir` 等） | `asset!` 资产打包 + content-hash 服务、字体、Iconify 图标、可选 Tailwind |
| 会话/认证 | 自选 `tower-sessions` / `axum-login` 等 | 内置 cookie、session（bring-your-own-storage） |
| 其他内置 | 无（保持最小） | SSE、WebSocket、htmx/Datastar/Alpine AJAX、OpenAPI、sitemap、本地化（Fluent）、邮件、Topcoat UI 组件库、`topcoat` CLI（`new`/`fmt`/`ui`） |

## 哪个比较合适？

**短答案：**

- 做 **JSON API / 微服务 / 需要精细控制 tower 中间件 / 已有 tower 生态 / 追求稳定** → 选 **axum**。
- 做 **全栈服务端渲染网站 / 表单 / 交互页面，想少写样板、不想再单独搭前端**，且能接受实验性版本 → 选 **topcoat**。
- 两者并不互斥：topcoat 的 `tower` feature 提供 `TowerService`，可以把 topcoat 挂进 axum 路由（本 workspace 的 `topcoat_gpui_example` 就是这么做的：axum 负责 `/healthz` 等 API，`fallback_service` 交给 topcoat）。

### 选 axum 的情况

1. 你的核心产出是 HTTP API（REST/gRPC-Web/JSON），HTML 渲染不是重点。
2. 你需要极致的底层控制：自定义 `tower::Service` / `Layer`、流式响应、`FromRequest` / `FromRequestParts`、hyper 级别的调优。
3. 团队对 tower 生态熟悉，想自由挑选模板引擎、会话、鉴权、限流等组件。
4. 生产稳定性是第一优先级；不能接受框架本身频繁 breaking change。
5. 和已有的 axum 项目集成，或挂载在其他 tower 应用里。

### 选 topcoat 的情况

1. 你在做**服务端渲染的网站/全栈应用**，页面、表单、局部交互都由服务器产出 HTML。
2. 你希望少搭建：不想再维护独立前端项目、API 层、构建管线；轻交互用 `$(...)` / `#[shard]` 就能完成，不需要 wasm。
3. 你想要类型安全的模板、异步组件、模块化路由、资产/Tailwind/会话/OpenAPI 等“开箱即用”的整套方案。
4. 项目处于快速迭代阶段，能接受 early-stage 框架的 API 变动，并愿意跟随版本升级。
5. 你喜欢 Django / Rails / Next.js 风格的“全栈电池齐全”体验，但要在 Rust 里做。

### 不适合的情况

- 不要为了“更强”而选 topcoat 做纯 JSON API —— axum 更小、更稳。
- 不要在要求长期 API 稳定、不能随意升级依赖的大型生产系统里，不锁版本地用 topcoat（目前 `0.12.1`，明确标注 experimental）。
- 如果你已经有成熟的前端 SPA + axum API 架构，引入 topcoat 的全栈模式收益不大。

## 参考

- axum: <https://github.com/tokio-rs/axum>
- topcoat: <https://github.com/tokio-rs/topcoat>（本机缓存源码见 `~/.cargo/registry/src/*/topcoat-0.12.1`，文档在其 `docs/` 目录）
- 本 workspace 既有示例：`topcoat_example`、`topcoat_gpui_example`（topcoat 挂进 axum）
