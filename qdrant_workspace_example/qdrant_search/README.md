# Qdrant 单节点 / Cluster 搜索示例

## 单节点

```sh
./qdrant-docker.sh start
cargo run -p qdrant_search
```

## 三节点 Cluster

```sh
docker compose -f docker-compose.cluster.yml up -d
curl -fsS http://localhost:16333/cluster
curl -fsS http://localhost:16343/cluster
curl -fsS http://localhost:16353/cluster
```

等待三个节点的 `/cluster` 均显示 `status: enabled`、三个 peers、非空
`raft_info.leader`，且 `/readyz` 返回成功后运行：

```sh
QDRANT_CLUSTER=true cargo run -p qdrant_search
```

默认创建 `cluster_test` collection，包含 6 个逻辑分片、每个分片 2 个副本，
总计 12 个物理分片。客户端连接任意节点，由 Qdrant 路由跨节点请求。
示例会输出 collection 配置、分片分布、写入并等待完成，然后查询向量和 payload。

| 节点 | REST / Dashboard | gRPC |
| --- | --- | --- |
| node1 | http://localhost:16333 | http://localhost:16334 |
| node2 | http://localhost:16343 | http://localhost:16344 |
| node3 | http://localhost:16353 | http://localhost:16354 |

节点使用独立 Docker volume；6335 仅用于 Docker 网络内的节点通信。
集群端口与原单节点端口不冲突，存储也相互独立。

```sh
# 从第二个节点执行同一示例
QDRANT_CLUSTER=true QDRANT_URL=http://localhost:16344 cargo run -p qdrant_search
# 查看分片副本分布
curl -fsS http://localhost:16333/collections/cluster_test/cluster
# 日志、停止（保留数据）、重新启动
docker compose -f docker-compose.cluster.yml logs -f
docker compose -f docker-compose.cluster.yml down
docker compose -f docker-compose.cluster.yml up -d
```

## 环境变量

| 变量 | 单节点默认值 | Cluster 默认值 |
| --- | --- | --- |
| `QDRANT_CLUSTER` | `false` | 设置为 `true` 或 `1` |
| `QDRANT_URL` | `http://localhost:6334` | `http://localhost:16334` |
| `QDRANT_COLLECTION` | `test` | `cluster_test` |
| `QDRANT_SHARD_NUMBER` | `1` | `6` |
| `QDRANT_REPLICATION_FACTOR` | `1` | `2` |
| `QDRANT_WRITE_CONSISTENCY_FACTOR` | `1` | `1` |
| `QDRANT_IMAGE`（Compose） | — | `qdrant/qdrant:v1.20.0` |

分片、副本和写入一致性参数必须为正整数；写入一致性不能大于副本数。
副本数应不超过实际运行的节点数。默认写入一致性为 1，可按需求提高为 2。
`QDRANT_CLUSTER` 选择应用端配置，服务端 cluster 模式由 Compose 开启。
客户端当前使用一个入口地址；入口故障时需切换 `QDRANT_URL` 或部署负载均衡器。

重复运行会复用 collection，并 upsert 示例点 ID 0，不会删除 collection。
已有 collection 的分片、副本或写入一致性与配置不符时会报错；可指定新的
`QDRANT_COLLECTION`。已有单节点数据不会自动迁移到这个独立集群。
自托管集群扩容后，已有分片需手动迁移/复制，不能仅修改配置来实现重平衡。

部署方式与分片、副本语义参见 [Qdrant 官方分布式部署文档](https://qdrant.tech/documentation/scaling/distributed_deployment/)。

## 验证

```sh
cargo test -p qdrant_search
cargo clippy -p qdrant_search --all-targets -- -D warnings
docker compose -f docker-compose.cluster.yml config --quiet
```
