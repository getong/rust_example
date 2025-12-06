# grpc-graphql-gateway-example

示例项目：启动一个 gRPC Greeter 服务，并通过 `grpc_graphql_gateway` 动态暴露 GraphQL 接口（含 Subscription）。运行后会同时起 gRPC 和 GraphQL 两个端口，便于快速体验。

## 运行

```bash
cargo run
```

启动后日志会显示：

- gRPC: `http://127.0.0.1:50051`
- GraphQL: `http://127.0.0.1:8000/graphql`（Playground 页面）
- WebSocket: `ws://127.0.0.1:8000/graphql/ws`（用于 Subscription）

> 如果在线下载依赖受限，可先运行 `cargo check --offline`。

## GraphQL 用法

默认生成的 schema 来自 `proto/greeter.proto` 上的 `(graphql.*)` 选项，主要操作：

- Query: `hello`
- Mutation: `updateGreeting`
- Subscription: `streamHello`
- Resolver: `user`（示例用于嵌套解析）
- Mutations with bytes: `uploadAvatar` / `uploadAvatars`

### 1. Query 示例

```bash
curl -XPOST -H 'content-type: application/json' \
  --data '{"query":"{ hello(name:\"Rust\") { message meta { correlationId from { id displayName trusted } } } }"}' \
  http://127.0.0.1:8000/graphql
```

### 2. Mutation 示例

```bash
curl -XPOST -H 'content-type: application/json' \
  --data '{"query":"mutation { updateGreeting(input:{ name:\"Rust\", salutation:\"Ahoy\" }) { message } }"}' \
  http://127.0.0.1:8000/graphql
```

### 3. Subscription 示例（streamHello）

在 Playground / GraphiQL 订阅：

```graphql
subscription {
  streamHello(name: "Rust") {
    message
    meta { correlationId }
  }
}
```

使用 `wscat` 也可以：

```bash
wscat -c ws://127.0.0.1:8000/graphql/ws
# 然后发送 payload（GraphQL over WebSocket-transport payload）
{"type":"connection_init"}
{"id":"1","type":"start","payload":{"query":"subscription { streamHello(name:\"Rust\") { message } }"}}
```

### 4. 文件/字节上传

`uploadAvatar` 和 `uploadAvatars` 接收 `bytes` 字段；通过 GraphQL 需发送 base64（GraphQL JSON 字符串）：

```bash
curl -XPOST -H 'content-type: application/json' \
  --data '{"query":"mutation { uploadAvatar(input:{ userId:\"u1\", avatar:\"SGVsbG8sIEdSUEMh\" }) { userId size } }"}' \
  http://127.0.0.1:8000/graphql
```

## 目录

- `proto/`：gRPC + GraphQL 注解的 proto 定义
- `src/generated/`：通过 `build.rs` 生成的 protobuf/descriptor
- `src/main.rs`：示例 gRPC 实现 + GraphQL Gateway 启动入口

欢迎按需替换后端地址、扩展 proto，再运行 `cargo run` 体验。
