# Agones Allocator：Rust + tonic

通过 **Agones Allocator Service** 的 `allocation.AllocationService/Allocate`
进行游戏服务器分配，不通过 Kubernetes API 创建 `GameServerAllocation`。

## 范围与结构

- `agones_tonic_client` 同时提供可复用的 Rust 库和命令行程序。
- `build.rs` 使用 `tonic-prost-build`、`prost` 和随依赖分发的 `protoc` 编译协议；无需手动安装 protoc，编译脚本不下载协议。
- 官方 `allocation.proto` 固定为 Agones **v1.62.0**；其 HTTP/OpenAPI 注解依赖也已保存，来源见 `agones_tonic_client/proto/README.md`。
- 默认强制 HTTPS/mTLS，验证服务端 CA、证书 SAN，并发送客户端证书；没有跳过证书验证的开关。
- 每个客户端复用一个 HTTP/2 Channel；克隆客户端不重新建连，也不通过全局 Mutex 串行化 RPC。
- 所有客户端克隆共享并发上限；每次分配的超时涵盖等待并发许可和 RPC，并向服务端发送剩余的 `grpc-timeout`。
- **没有自动重试**。分配不是天然幂等操作，超时或断连不代表服务端没有成功分配。

Allocator 只负责选择和分配 GameServer，不提供 Fleet 创建、扩缩容、删除或 GameServer 生命周期管理。
这些工作仍由 Agones/Kubernetes 控制器、FleetAutoscaler 和游戏服内的 Agones SDK 完成。
本项目没有实现 REST 客户端，也不要求匹配服务持有 Kubernetes kubeconfig。

## 1. 准备 Allocator 与证书

示例 Helm values 位于 `deploy/allocator-values.yaml`，保留 mTLS 并关闭 REST，
让 gRPC 独占端口，避免默认 gRPC/REST 共享端口的多路复用路径。
请合并到自己的安装配置，不要不加审查地覆盖现有集群设置。例如新安装：

```bash
helm repo add agones https://agones.dev/chart/stable
helm repo update
helm install agones agones/agones --version 1.62.0 \
  --namespace agones-system --create-namespace \
  -f deploy/allocator-values.yaml
kubectl get service agones-allocator -n agones-system
```

需要先有目标 namespace、Fleet 以及处于 `Ready` 状态的 GameServer。
仅部署 Allocator 不会自动创建可分配的游戏房间。

如果使用 Helm 默认生成的客户端证书与 Secret，可以从**拥有管理权限的环境**导出：

```bash
mkdir -p certs
umask 077
kubectl get secret allocator-client.default -n default \
  -o jsonpath='{.data.tls\.crt}' | base64 -d > certs/client.crt
kubectl get secret allocator-client.default -n default \
  -o jsonpath='{.data.tls\.key}' | base64 -d > certs/client.key
kubectl get secret allocator-tls-ca -n agones-system \
  -o jsonpath='{.data.tls-ca\.crt}' | base64 -d > certs/ca.crt
chmod 600 certs/client.key
```

Secret 名称和所在 namespace 取决于你的安装方式；自定义证书时请改用实际 Secret。
`ca.crt` 必须信任 **Allocator 服务端证书**；客户端证书的签发 CA 则必须在目标
Allocator 的 `allocator-client-ca` 授权列表中。这是两个不同方向的信任关系。
生产环境建议由证书管理系统签发并挂载凭据，不要把私钥提交到仓库。
`certs/`、`*.key` 和 `*.pem` 已加入 `.gitignore`。

默认安装的服务端证书不一定包含后来分配的 LoadBalancer IP。
应使用带正确 DNS/IP SAN 的证书，或根据 Agones 安装说明配置固定 LoadBalancer IP 后更新证书。
`--server-name` 只指定预期证书身份和 SNI，**不会绕过 SAN 校验**。
CA、客户端证书与私钥在连接时读取；轮换后需要新建客户端并切换流量。

## 2. 单次分配

要求支持 Rust 2024 edition 的较新 Rust 工具链；依赖的最低版本以 Cargo 解析结果为准。首次构建需要下载 Cargo 依赖。

```bash
export AGONES_ALLOCATOR_ENDPOINT=https://allocator.example.com:443
export AGONES_CA_CERT="$PWD/certs/ca.crt"
export AGONES_CLIENT_CERT="$PWD/certs/client.crt"
export AGONES_CLIENT_KEY="$PWD/certs/client.key"

cargo run -p agones_tonic_client -- \
  --namespace default \
  --fleet simple-game-server \
  --annotation example.com/match-id=match-001
```

也可以通过 `--endpoint`、`--ca-cert`、`--client-cert`、`--client-key` 设置连接信息；
命令行参数优先于环境变量。直连 IP 或本地端口转发时，可以额外传入
`--server-name allocator.example.com`，其值必须匹配实际证书 SAN。

默认只匹配 `Ready` 且 `agones.dev/fleet=<fleet>` 的 GameServer。
`--selector KEY=VALUE` 可重复传入以增加 AND 条件。
`--scheduling packed|distributed` 选择调度策略；`--label` 和 `--annotation`
可重复传入，用于在分配时更新服务器元数据。

stdout 每行输出一个成功分配的 JSON 对象，字段采用 Rust 的 snake_case，例如：

```json
{"game_server_name":"simple-game-server-abcde","ports":[{"name":"default","port":7777}],"address":"203.0.113.10","addresses":[],"node_name":"node-1","source":"","metadata":null,"counters":{},"lists":{}}
```

应按端口名称选择 `ports`，不要假定只有一个端口。`source` 是远端 Allocator 的来源信息，
不是保证存在的集群名称；`node_name` 也不等于集群名称。
错误写入 stderr，存在任意失败时进程非零退出。
输出是本工具的 JSON，而不是 gRPC-Gateway 的 REST 请求/响应格式。

## 3. 并发分配

```bash
cargo run --release -p agones_tonic_client -- \
  --fleet simple-game-server \
  --count 100 \
  --max-in-flight 32 \
  --timeout-ms 5000 \
  --connect-timeout-ms 5000
```

`--count 100` 是 **100 次独立分配**，不是给同一场比赛重试 100 次，也不是向一个房间添加 100 名玩家。
输出顺序是完成顺序。CLI 采用有界并发，不会一次创建所有分配任务。
批量中部分请求失败时会继续处理剩余请求，保留并输出已成功的分配，不执行回滚。
高并发不等于固定延迟保证，实际延迟与 Ready 容量、Allocator 扩容、网络和调度配置有关。

## 4. 跨集群分配

本客户端支持 `multiClusterSetting` 和 `policySelector`。策略由 Allocator 执行，
不是客户端向多个集群广播同一场比赛的请求。

1. 先确认每个目标集群能单独完成 mTLS 分配。
2. 修改 `deploy/multi-cluster-policy.yaml` 的 namespace、远端地址和集群标识。
3. 在**入口集群策略所在 namespace** 创建 `allocator-client-to-remote` Secret，
   包含 `tls.crt`、`tls.key`、`ca.crt`。远端必须信任此客户端的 CA；服务端 SAN 必须匹配远端地址。
4. 在入口集群应用策略，然后启用跨集群请求：

```bash
kubectl create secret generic allocator-client-to-remote -n default \
  --from-file=tls.crt=/secure/remote-client.crt \
  --from-file=tls.key=/secure/remote-client.key \
  --from-file=ca.crt=/secure/remote-server-ca.crt
kubectl apply -f deploy/multi-cluster-policy.yaml

cargo run -p agones_tonic_client -- \
  --namespace default \
  --fleet simple-game-server \
  --multi-cluster \
  --policy-label allocation-pool=production \
  --timeout-ms 35000
```

示例本地策略优先级为 1，远端为 2；数字越小越优先，同优先级按权重选择。
本地策略不配置 `allocationEndpoints`。远端示例 IP 是占位符，必须替换为实际 Allocator 地址（标准端口 443），并确保证书包含该 IP 的 SAN。
本地和远端要有与请求 Fleet 标签一致的可分配 GameServer。
启用跨集群后，请求 namespace 用于查找策略，实际目标 GameServer namespace 来自策略的 `connectionInfo.namespace`。
`--policy-label` 只筛选策略标签，与 `--selector` 的 GameServer 标签不同。
不传策略标签时匹配全部策略；**只开启开关不会替你创建策略、证书信任或网络连接**。

Agones 当前跨集群文档建议新架构评估 Service Mesh 统一分配入口；这里同时支持已有的
`GameServerAllocationPolicy` 路径。使用 Mesh 时根据其路由与 mTLS 设计配置入口，
不必一律开启 `--multi-cluster`。不要混淆 Mesh 跨集群路由与 Allocator 内置策略转发。

## 5. 嵌入 Matchmaker

在进程启动时建立一个 `AllocatorClient`，为匹配任务克隆它，不要每次请求重新读证书、建立 TLS 连接。

```rust,no_run
use agones_tonic_client::{AllocatorClient, AllocatorConfig};
use agones_tonic_client::allocation::{AllocationRequest, GameServerSelector};

async fn allocate_match() -> Result<(), Box<dyn std::error::Error>> {
    let config = AllocatorConfig::new(
        "https://allocator.example.com:443",
        "certs/ca.crt",
        "certs/client.crt",
        "certs/client.key",
    );
    let client = AllocatorClient::connect(config).await?;
    let request = AllocationRequest {
        namespace: "default".into(),
        game_server_selectors: vec![GameServerSelector {
            match_labels: [("agones.dev/fleet".into(), "simple-game-server".into())].into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let server = client.clone().allocate(request).await?;
    println!("{}: {:?}", server.address, server.ports);
    Ok(())
}
```

库公开完整生成协议，支持有序候选 selectors、match expressions、Counters/Lists 以及优先级等高级字段；
是否可用取决于目标集群的 Agones 版本。`AllocatorError::Rpc` 保留 `tonic::Status`，调用方可检查 `code()`。
匹配层应持久化 match/session 到已分配 GameServer 的映射，并自行处理不确定结果。
写入 match-id annotation **不等于** Allocator 实现了幂等键。

## 验证与排查

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p agones_tonic_client -- --help
```

- TLS 失败：检查服务端 CA、SAN/SNI、客户端证书授权、有效期与证书链；负载均衡器必须正确传递 HTTP/2 和客户端认证。
- 无可用房间：检查 namespace、Fleet 标签和 Ready 数量；跨集群还要检查策略标签和远端信任。
- `DeadlineExceeded` / `Unavailable` / 断连：结果可能不确定，不要盲目重试；优先核查匹配记录及服务端分配状态。
- 超时包含并发排队；跨集群超时应结合入口 Allocator 的远端超时配置设置，而不是照搬单集群值。
- 此仓库不包含真实集群凭据；本地编译验证不能替代真实集群和并发压测。

协议与安装行为依据 Agones v1.62.0 官方仓库中的
`proto/allocation/allocation.proto`、`site/content/en/docs/Advanced/allocator-service.md`、
`site/content/en/docs/Advanced/multi-cluster-allocation.md` 和 `install/helm/agones/values.yaml`。
