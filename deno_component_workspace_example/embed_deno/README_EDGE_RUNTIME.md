# Edge Runtime 演示文档

这个项目实现了一个基于 edge-runtime 架构的 Deno 运行时演示，直接在 `src/main.rs` 中实现了完整的架构，支持NPM包集成和TypeScript模块执行。

## 🚀 快速开始

### 1. 编译项目

```bash
# 进入项目目录
cd /Users/gerald/personal_infos/rust_example/deno_component_workspace_example/embed_deno

# 编译项目
cargo build
```

### 2. 运行 Edge Runtime 演示

#### 默认模式（推荐）- Daemon 模式
```bash
# 直接运行二进制文件，默认启动 edge-runtime 演示
../target/debug/embed_deno
```

#### 明确指定演示模式
```bash
# 使用 --edge-demo 参数明确启动演示
../target/debug/embed_deno --edge-demo
```

## 📋 运行输出示例

### 基础运行输出
```
Starting Edge Runtime Demo with real architecture...
Edge runtime server created, starting to listen...
Edge runtime listening on: 127.0.0.1:9999
Simulating incoming request: Request #1
Starting main worker for: "./simple_main.ts"
Edge runtime demo completed successfully
```

### 完整NPM包集成输出（使用main.ts）
```bash
# 切换到完整NPM包版本
# 在 src/main.rs 中将 "./simple_main.ts" 改为 "./main.ts"
```

期望输出：
```
🚀 Main TypeScript module loaded with NPM packages!

📦 Testing NPM packages:
📌 Generated ID (nanoid): XYZ123ABC
📅 Formatted date (date-fns): 2024-01-15 14:30:25
🔤 Capitalized (lodash): Hello world from edge runtime
🐪 CamelCase (lodash): helloWorldFromEdgeRuntime
✅ Zod validation passed: { id: "...", name: "Edge Runtime User", ... }
📦 All NPM packages tested successfully!

🧪 Testing enhanced functions:
✖️ Calculator [ABC123]: 7 × 8 = 56
➗ Calculator [XYZ789]: 100 ÷ 4 = 25
➕ Add operation [DEF456]: 15 + 25 = 40
🎉 Greet called [GHI012] at 14:30:25: Welcome, Edge Runtime User!

✨ Main TypeScript module initialization complete with NPM packages!
```

## 🏗️ 架构特性

### 核心组件

1. **EdgeServerFlags** - 服务器配置管理
2. **EdgeSupervisorPolicy** - Worker 监督策略（PerWorker/PerRequest）
3. **EdgeWorkerPoolPolicy** - Worker 池策略配置
4. **EdgeMainWorkerSurface** - 主 Worker 表面，管理 JS 运行时
5. **EdgeServer** - 主服务器，协调所有组件
6. **EdgeBuilder** - 构建器模式，用于配置和创建服务器

### 技术实现

- **Tokio 运行时**: 使用 `current_thread` 运行时和 `LocalSet`
- **MPSC Channels**: Worker 间通信使用 `mpsc::unbounded_channel`
- **Deno JS 运行时**: 集成真正的 `JsRuntime`，支持 TypeScript 模块
- **异步请求处理**: 使用 `tokio::select!` 处理请求和取消信号
- **优雅关闭**: 实现 cancellation token 系统

## 📦 NPM 包集成

### 支持的 NPM 包

本项目集成了以下NPM包，展示了在edge-runtime中使用第三方依赖的能力：

1. **nanoid@5.0.4** - 生成唯一ID
   ```typescript
   import { nanoid } from "npm:nanoid@5.0.4";
   const id = nanoid(); // 生成随机ID
   ```

2. **date-fns@3.6.0** - 日期时间处理
   ```typescript
   import { format, parseISO } from "npm:date-fns@3.6.0";
   const formatted = format(new Date(), "yyyy-MM-dd HH:mm:ss");
   ```

3. **lodash-es@4.17.21** - 实用工具函数
   ```typescript
   import { capitalize, camelCase } from "npm:lodash-es@4.17.21";
   const text = capitalize("hello world");
   const camel = camelCase("hello world");
   ```

4. **zod@3.22.4** - 运行时类型验证
   ```typescript
   import { z } from "npm:zod@3.22.4";
   const UserSchema = z.object({
     name: z.string(),
     age: z.number().positive()
   });
   ```

### NPM 包功能演示

运行时会自动执行以下NPM包功能测试：
- ✅ ID 生成和验证
- ✅ 日期格式化
- ✅ 文本处理（大写、驼峰命名）
- ✅ 数据验证（Zod schema）
- ✅ 增强的计算器功能
- ✅ 请求处理与响应格式化

## 📁 项目结构

```
embed_deno/
├── src/
│   └── main.rs              # 包含完整的 edge-runtime 架构实现
├── main.ts                  # 完整TypeScript入口文件（包含NPM包）
├── simple_main.ts           # 简化版TypeScript文件（基础测试）
├── Cargo.toml               # Rust 项目配置
└── README_EDGE_RUNTIME.md   # 本文档
```

## 🔧 配置说明

### 服务器配置

- **端口**: 127.0.0.1:9999
- **Worker 策略**: PerWorker 模式
- **最大并行度**: 2 个 worker
- **请求超时**: 30 秒
- **优雅关闭时间**: 10 秒

### Worker 配置

- **模块缓存**: 启用
- **服务路径**: `./simple_main.ts` (默认) 或 `./main.ts` (完整功能)
- **环境变量**: 空（可扩展）
- **NPM 包支持**: 通过 `npm:` 前缀导入

### 模式切换

#### 简化模式（当前默认）
```rust
// src/main.rs 第938行
let main_service_path = PathBuf::from("./simple_main.ts");
```

#### 完整NPM包模式
```rust
// src/main.rs 第938行
let main_service_path = PathBuf::from("./main.ts");
```

## 🎯 核心流程

1. **启动**: 创建 Tokio 运行时和 LocalSet
2. **构建**: 使用 Builder 模式配置服务器
3. **Worker 初始化**: 启动主 Worker 并加载 TypeScript 模块
4. **请求处理**: 通过 MPSC channel 处理模拟的 HTTP 请求
5. **响应返回**: 使用 oneshot channel 返回处理结果
6. **优雅关闭**: 取消所有 worker 并等待完成

## 🔄 与 edge-runtime 的对比

| 特性 | edge-runtime | 本实现 |
|------|-------------|--------|
| Tokio 运行时 | ✅ | ✅ |
| MPSC Channels | ✅ | ✅ |
| Worker 管理 | ✅ | ✅ |
| Deno 集成 | ✅ | ✅ |
| 构建器模式 | ✅ | ✅ |
| 优雅关闭 | ✅ | ✅ |
| HTTP 服务器 | ✅ | 🚧 (模拟) |
| TLS 支持 | ✅ | ❌ |

## 🛠️ 开发说明

### 修改 TypeScript 代码

#### 基础功能（simple_main.ts）
```typescript
// simple_main.ts - 简化版本
console.log("🚀 Loading simple main.ts...");

globalThis.handleRequest = (req: string) => {
  console.log(`[Simple TS] Processing request: ${req}`);
  return `Simple response: ${req} at ${new Date().toISOString()}`;
};
```

#### 完整功能（main.ts）
```typescript
// main.ts - 包含NPM包的完整版本
import { nanoid } from "npm:nanoid@5.0.4";
import { format } from "npm:date-fns@3.6.0";
import { capitalize, camelCase } from "npm:lodash-es@4.17.21";
import { z } from "npm:zod@3.22.4";

globalThis.handleRequest = (req: string) => {
  const requestId = nanoid();
  const timestamp = format(new Date(), "yyyy-MM-dd'T'HH:mm:ss.SSSxxx");
  const processedReq = capitalize(req);
  
  return JSON.stringify({
    id: requestId,
    timestamp: timestamp,
    originalRequest: req,
    processedRequest: processedReq,
    message: "Request processed with NPM packages"
  }, null, 2);
};
```

### 添加新的 NPM 包

1. 在 `main.ts` 中添加导入：
   ```typescript
   import { someFunction } from "npm:package-name@version";
   ```

2. 在 `testNpmPackages()` 函数中添加测试：
   ```typescript
   function testNpmPackages() {
     // 测试你的新包
     const result = someFunction();
     console.log(`📦 New package result: ${result}`);
   }
   ```

### 添加新的 Worker 策略

在 `EdgeSupervisorPolicy` 枚举中添加新策略：

```rust
enum EdgeSupervisorPolicy {
    PerWorker,
    PerRequest { oneshot: bool },
    // 添加新策略
    Custom { /* 参数 */ },
}
```

### 扩展服务器配置

修改 `EdgeServerFlags` 结构体来添加新配置：

```rust
struct EdgeServerFlags {
    no_module_cache: bool,
    graceful_exit_deadline_sec: u64,
    tcp_nodelay: bool,
    request_wait_timeout_ms: Option<u64>,
    // 添加新配置
    // custom_option: bool,
}
```

## ⚡ 快速切换指南

### 切换到完整NPM包模式

1. **修改源码**：
   ```bash
   # 编辑 src/main.rs 第938行
   sed -i 's/simple_main\.ts/main.ts/' src/main.rs
   ```

2. **重新编译**：
   ```bash
   cargo build
   ```

3. **运行完整版本**：
   ```bash
   ../target/debug/embed_deno
   ```

### 切换回简化模式

1. **恢复源码**：
   ```bash
   # 编辑 src/main.rs 第938行
   sed -i 's/main\.ts/simple_main.ts/' src/main.rs
   ```

2. **重新编译并运行**：
   ```bash
   cargo build && ../target/debug/embed_deno
   ```

## 🐛 故障排除

### 常见问题

1. **编译错误**: 确保安装了正确版本的 Rust 和相关依赖
2. **模块加载失败**: 检查 TypeScript 文件是否存在且语法正确
3. **NPM包下载失败**: 确保网络连接正常，Deno会自动下载npm包
4. **Worker 通信问题**: 查看日志中的 channel 错误信息

### 调试模式

使用以下命令启用详细日志：

```bash
RUST_LOG=debug ../target/debug/embed_deno
```

### NPM包相关问题

1. **包版本冲突**: 检查 `main.ts` 中的版本号
2. **导入错误**: 确保使用 `npm:` 前缀
3. **权限问题**: 确保有网络访问权限下载npm包

## 📊 功能特性对比

| 功能 | simple_main.ts | main.ts (完整版) |
|------|----------------|------------------|
| 基础 TypeScript 支持 | ✅ | ✅ |
| 控制台输出 | ✅ | ✅ |
| 请求处理 | ✅ | ✅ |
| NPM 包支持 | ❌ | ✅ |
| 唯一ID生成 | ❌ | ✅ (nanoid) |
| 日期格式化 | ❌ | ✅ (date-fns) |
| 文本处理 | ❌ | ✅ (lodash) |
| 数据验证 | ❌ | ✅ (zod) |
| 增强的响应格式 | ❌ | ✅ |
| 自动功能测试 | ❌ | ✅ |
| 启动速度 | 🚀 快 | ⚡ 稍慢（需下载包） |
| 内存使用 | 💾 低 | 💾 中等 |

## 🎯 使用建议

- **开发调试**: 使用 `simple_main.ts` 进行快速测试
- **功能演示**: 使用 `main.ts` 展示完整的NPM包集成
- **生产环境**: 根据实际需求选择合适的版本

## 📚 参考资料

- [Edge Runtime 项目](https://github.com/supabase/edge-runtime)
- [Deno Core 文档](https://docs.rs/deno_core/)
- [Tokio 文档](https://tokio.rs/)
- [Deno NPM 包支持](https://deno.land/manual/node/npm_specifiers)

## ⚖️ 许可证

本项目遵循与原 Deno 项目相同的 MIT 许可证。