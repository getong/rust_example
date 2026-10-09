# Topcoat × GPUI 抗量子加密协作

浏览器 TypeScript 与原生 GPUI 客户端共享计数器、待办、JSON 回显、资料表单、配色实验室。服务端业务逻辑由 Topcoat 实现，桌面项目为 `../../gpui_workspace_example/gpui_topcoat_example`，双方共用其 `protocol` crate。

所有业务请求/响应经 ML-KEM-1024 + ML-DSA-87 + HKDF-SHA256 + AES-256-GCM 加密。Rust 使用 `aws-lc-rs`，浏览器使用固定版本 noble-post-quantum 和 WebCrypto。参考 `aws_lc_rs_workspace_example/encrypt_file` 的算法选型。

## 启动

需要 Rust、Bun；Cargo 构建时自动执行冻结锁文件安装、TypeScript 类型检查和网页打包。

无需 keygen、公钥文件或密钥环境变量。每次应用握手自动生成客户端 ML-KEM 密钥和服务端 ML-DSA 密钥，公钥在握手消息中动态交换，私钥仅留在内存。

```sh
# Axum 工作区
cargo run -p topcoat_gpui_example
# GPUI 工作区
cargo run -p gpui_topcoat_example
```

浏览器访问 <http://127.0.0.1:3000>，可切换 `/todos`、`/echo`、`/profile`、`/studio`。默认 `HOST=127.0.0.1 PORT=3000`，桌面可用 `TOPCOAT_URL` 指定根地址。远程浏览器必须 HTTPS；仅本机开发允许 localhost HTTP。

网页初始 HTML 不包含业务快照，加载后从加密通道同步；发布后两端每两秒检查最新状态。所有历史保存在服务端内存，重启清空。修改失败不会自动重试，需先刷新确认是否已执行。

## 网关

| 外部路径 | 功能 |
| --- | --- |
| `POST /pq/handshake` | 临时 ML-KEM 公钥交换、返回本次新生成的 ML-DSA 公钥和签名 |
| `POST /pq/exchange` | 一次性会话 AES-GCM 请求/响应 |
| `/api/*` | 外部返回 403；只在通过解密验证后内部调用 |

业务 envelope 内仍使用 `/api/counter`、`/api/demos`、`/api/todos`、`/api/echo`、`/api/profile`、`/api/studio`。资料表单现在使用 JSON `{username, age}`。既有输入校验、互斥状态和业务错误保留；错误内容也加密。

单会话仅一个请求/响应，60 秒过期，最多 1024 个未使用会话，防止相同请求重复执行。握手公钥被篡改、握手签名错误、密文篡改均失败关闭，不降级成明文。

协议、安全边界及浏览器信任起点见 [客户端 SECURITY.md](../../gpui_workspace_example/gpui_topcoat_example/SECURITY.md)。动态签名公钥的信任来自 HTTPS，签名本身不证明服务器身份；服务端身份验证不等于用户登录；示例没有客户端授权、磁盘数据加密，也未做独立密码协议审计。网页首次加载依赖 HTTPS 的认证信任，不能宣称普通 HTTPS 引导就具有完全抗量子的身份保证。

## 验证

```sh
# Axum 工作区
cargo test -p topcoat_gpui_example
cargo build -p topcoat_gpui_example

# 当前网页项目
bun test tests/*.test.ts
bun run build

# GPUI 工作区
cargo test -p topcoat_gpui_protocol
cargo test -p gpui_topcoat_example --all-targets
cargo build -p gpui_topcoat_example --no-default-features --bin topcoat-smoke --bin studio-smoke
python3 gpui_topcoat_example/scripts/smoke.py

# 当前网页项目：真实浏览器与 studio-smoke 交互
bun run test:e2e
```

联调/E2E 无需生成或配置密钥文件。`scripts/pq-smoke.ts` 验证 noble/WebCrypto ↔ AWS-LC 的全部业务 API、动态公钥篡改、加密业务错误、明文拒绝。E2E 验证网页发布、原生桌面同步、本地草稿保留及被篡改响应后的恢复。

## 实时补全

在 `/todos` 输入 `学`、`rust` 或 `kf`，在 `/profile` 输入 `张`、`zxm` 或 `Ali`，180ms 防抖后自动显示最多 6 条服务端建议。↑↓ 选择、Enter 填入、Esc/失焦关闭，鼠标点击也可填入，不会自动提交。网页突出显示匹配文字，区分常用建议和双端共享记录，提供加载、无匹配和断线提示。

`POST /api/suggestions` 是加密网关内部的只读查询，接收 `{kind:"todo"|"profile",query:string}`，返回 `{query,items:[{value,detail,source:"shared"|"suggested"}]}`。查询最多 120 字，不允许控制字符。服务端结合常用词和当前共享待办/联系人，按前缀、包含、常用词拼音别名匹配排序去重。用户自建记录仅按实际文字搜索。没有输入内容写入、额外持久化或 WebSocket 连接。

桌面和网页都取消过时查询，并防止迟到结果覆盖新输入。网页支持中文输入法组字和 combobox/listbox 可访问性属性。`tests/autocomplete.spec.ts` 验证中文输入、拼音、键盘/鼠标、旧响应、断线恢复及原生客户端写入后的共享建议；Rust 服务端测试验证排名去重、上限、参数校验和加密请求路径。
