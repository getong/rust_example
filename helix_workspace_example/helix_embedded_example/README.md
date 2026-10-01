# HelixDB 嵌入式 Rust 案例

参考 [Embedded database 官方文档](https://docs.helix-db.com/database/helix-db/start-here/local-development/embedded-database)，在 Rust 进程内运行 HelixDB。无需启动 HTTP 服务、Docker 或 Helix CLI。

## 运行

在本目录或父 Cargo workspace 中运行：

```bash
# 内存读写：创建 Ada 节点，校验节点数量增加 1
cargo run -p helix_embedded_example -- memory

# 磁盘读写：创建节点、关闭、重新打开，校验数量保持一致
cargo run -p helix_embedded_example -- disk

# 打开上一步数据库：读取数量，并验证写入被拒绝
cargo run -p helix_embedded_example -- reader
```

默认运行 `memory`。磁盘数据保存在本项目的 `data/`，数据库名称为 `embedded-demo`；此目录已加入 `.gitignore`，`disk` 会自动创建缺失的数据根目录。每次运行 `disk` 都新增一个 `EmbeddedDemoUser` 节点，因此再次执行会显示 `1 -> 2` 等累计数量。`reader` 必须在 `disk` 初始化数据库后运行。

可指定数据根目录（相对路径以命令执行目录为准），两个命令须使用相同路径：

```bash
cargo run -p helix_embedded_example -- disk /tmp/helix-embedded-demo
cargo run -p helix_embedded_example -- reader /tmp/helix-embedded-demo
cargo run -p helix_embedded_example -- --help
```

所有案例均显式调用 `client.close().await`，查询失败时也会尝试关闭。示例按顺序运行，不要同时对同一数据库运行多个 writer。

## 图数据库案例

新增四个独立内存案例：社交二跳发现、共同购买推荐、知识溯源、服务依赖影响分析。每个案例自动建图并校验结果。

```bash
cargo run -p helix_embedded_example -- graph-all
```

也可分别选择 `social`、`recommendation`、`knowledge`、`dependencies`。详见 [图数据库案例说明](docs/graph-examples.md)，包含关系图、预期结果和适用场景。代码位于 `src/graph_examples.rs`。

## 文件

- `src/main.rs`：可执行 Rust 案例，使用 `QueryRequest::write/read`、`write_batch/read_batch`、节点创建和计数查询。
- `docs/upstream/embedded-database.mdx`：官方嵌入式文档原文，包含 Rust、TypeScript、Go、Python 代码及缓存配置。
- `docs/upstream/rust-project-setup.mdx`：官方 Rust SDK 配套文档原文。
- `docs/upstream/LICENSE`：上游 Apache-2.0 许可证。
- `docs/upstream/README.md`：原文来源、版本与本地修改说明。

## 依赖版本

初始依赖 `helix-db = "3.0.0"` 的 registry 发布包未声明 `embedded` feature，不能直接使用文档中的 `cargo add helix-db --features embedded`。本项目通过 Git 依赖使用官方仓库的 `main` 分支，并启用 `embedded`：

```toml
helix-db = { git = "https://github.com/HelixDB/helix-db.git", branch = "main", features = ["embedded"] }
```

Cargo 自动查找仓库内 `sdks/rust` 的 `helix-db` 包，不能将 GitHub 的 `/tree/main/sdks/rust` 网页地址作为 Git 仓库 URL。实际构建提交由父 workspace 的 `Cargo.lock` 锁定。

项目使用 Rust 2024 edition；首次构建需要联网获取 HelixDB Git 源码及未缓存的引擎依赖（包括上游固定版本的 SlateDB Git 依赖），编译量明显大于仅使用 HTTP SDK。此项目属于父目录 workspace，依赖锁文件和默认 `target/` 均由父 workspace 管理。

## 与原文对应

`Client::open(HelixDbSource::Disk { .. })` 打开 writer，`Client::open_reader` 打开已有磁盘数据库的只读句柄；`InMemory` 用于临时数据。查询在进程内执行，不能附加 server 模式的 writer-only、warm-only 或 durability headers。

本地案例使用默认缓存。对象存储及自定义缓存配置保留在官方文档中，未在本案例连接 S3 服务；原文的缓存示例是 TypeScript，不能直接当作 Rust API 使用。

## 验证命令

```bash
cargo fmt -p helix_embedded_example -- --check
cargo check -p helix_embedded_example
# 按上述顺序运行 memory、disk、reader，案例内置行为校验。
```

2026-10-01 本机验证通过（Rust `1.101.0-nightly`）：

切换 Git 依赖后，`cargo build -p helix_embedded_example` 已通过；本次锁定提交为 `7b8b5dce320120fe66926312e5344fd6aceb3555`。以下运行结果来自此前相同提交的本地源码构建。

- `cargo build -p helix_embedded_example --offline`、格式检查通过。
- `memory`：节点数量 `0 -> 1`。
- `disk` 连续运行两次：分别为 `0 -> 1`、`1 -> 2`，每次关闭后重新打开数量保持不变。
- `reader`：读取到 2 个节点，写入返回 `writer_mode_required`，拒绝写入后数量不变。

本机 `data/` 已保留这两个验证节点。上游引擎有 4 条 `Atomic::fetch_update` 弃用警告，不影响构建和以上运行结果。
