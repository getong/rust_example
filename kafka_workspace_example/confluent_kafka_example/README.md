# confluent-kafka Rust 示例

参考 `/Users/gerald/test/rust/kafka-clients/rust/examples/producer.rs` 和
`consumer.rs`，使用 crates.io 的 `confluent-kafka` 1.0 原生 Rust 客户端。
需要 Rust 1.95+、Docker 和 Bash。

## 一键启动 Kafka，再运行收发演示

```bash
./start-kafka.sh
./run.sh
```

`start-kafka.sh` 使用本目录的 `Dockerfile` 构建 Kafka 镜像，启动单节点
KRaft broker/controller，等待就绪，并创建 3 分区、1 副本的
`confluent-rust-demo` topic，不需要 ZooKeeper。

`./run.sh` 默认执行 `demo`：发送 10 条带 key 的字符串消息，等待 broker 确认，
再用独立消费组从 earliest 开始读取，校验本次运行的 key 和 value，手动提交 offset。
重复运行时通过唯一 key 跳过历史消息；成功输出
`Demo verified: 10 messages produced and consumed.`，失败返回非零退出码。
历史数据较多时可换 topic 或增大消费超时。

## 分别运行生产者、消费者

```bash
./run.sh produce
./run.sh consume
```

生产者启用 LZ4 压缩、`acks=all`，逐条打印 topic、partition、offset。
消费者打印 key/value 和位置，每批处理完成后手动提交 offset，默认读满 10 条退出；
等待超过 60 秒则报错退出。独立消费者支持 Ctrl+C 关闭。
与参考项目一致，消费组使用 `group.protocol=consumer`（KIP-848）。
`demo` 每次使用新消费组；`consume` 使用固定消费组，从已提交的位置继续读取。
生产者和消费者在操作完成或失败后都会调用 `close()`。

也可以直接运行 Cargo：

```bash
cargo run -- demo
cargo run -- produce
cargo run -- consume
cargo run -- --help
```

## 线程池、多客户端订阅与顺序检查

```bash
./start-kafka.sh
KAFKA_WORKER_THREADS=4 KAFKA_CLIENTS=3 KAFKA_MESSAGE_COUNT=12 ./run.sh pool
KAFKA_WORKER_THREADS=4 KAFKA_CLIENTS=3 KAFKA_MESSAGE_COUNT=12 ./run.sh broadcast
./check-order.sh
```

程序使用固定大小的 Tokio 工作线程池。每个消费者任务持有自己的长生命周期
`KafkaConsumer`，客户端管理自己的 broker 连接，任务可在线程池内迁移。
线程数与客户端数独立配置，不将多个消费者放进同一个锁中轮流 `poll()`。
生产者复用一个客户端，启用幂等发送并逐条等待确认。

| 命令 | 消费组 | 同一 topic 的分配方式 | 完成条件 |
| --- | --- | --- | --- |
| `pool` | 所有客户端属于同一个独立测试组 | 按分区分摊消息 | 整个组收到本轮全部消息 |
| `broadcast` | 每个客户端属于不同的独立测试组 | 每个客户端读取全部消息 | 每个组均收到本轮全部消息 |

测试先将全局序号 `0..N-1` 轮流发送到 topic 的实际分区，每个分区使用固定 key，
记录生产者确认的 partition 和 offset，然后启动所有客户端订阅同一个 topic。
独立运行 ID 过滤历史数据；新消费组保证本轮消息可以完整读取。
客户端逐条处理本批数据，将观察结果送入有界通道；协调任务校验成功后才允许
客户端手动提交本批 offset。任务全部完成、失败、超时或 Ctrl+C 后统一关闭客户端，
即使某个客户端没有分配到分区，也不会等待它单独消费 N 条消息。
协调任务还会等待有本轮数据的客户端确认提交；提交遇到重分配、过期 member epoch
或可重试错误时，客户端继续 `poll()` 协调分配，再提交当前持有分区已处理的位置。
不会持有旧分区的 offset 盲目重试，失去所有权的分区可能由新客户端重放消息。

输出包含每个客户端的分区分配、处理消息时的线程 ID、消费条数，以及每个消费组的：

- `partition_order=PASS`：本轮每个分区的不同消息，其序号与 offset 均严格递增，且位置与生产确认一致。
- `global_sequence_order=NOT_ORDERED`：合并多个分区后，到达顺序与发送的全局序号不同；这是允许的结果。
- `global_sequence_order=ORDERED_THIS_RUN`：本次观察恰好有序，不代表多分区提供全局有序保证。
- `duplicates`：重复交付单独计数，完整性和顺序检查针对每条消息的首次观察，不声称 exactly-once。

Kafka 的顺序保证以分区为单位，同一消费组中一个分区在同一时刻由一个消费者负责。
多分区之间没有全局顺序保证；按单条消息再次并行派发业务处理，也可能打乱同一分区的完成顺序。
需要全局顺序时使用单分区，并串行处理；需要同一业务 key 有序时，将该 key 的消息路由到
同一分区、按分区串行处理，并在处理完成后提交 offset。多个生产者仍需自行定义业务序号的先后关系。

`check-order.sh` 依次验证：单分区配 3 个客户端、三分区配 3 个同组客户端、
三分区配 3 个不同组客户端、三分区配 5 个同组客户端，
以及单个工作线程调度三分区的 3 个同组客户端。
单分区同组消费只能有一个客户端处理数据；同组客户端数量多于分区时会有空闲客户端。
脚本使用 `confluent-rust-order-single` / `confluent-rust-order-multi` 两个持久化测试 topic，
可通过 `KAFKA_ORDER_TOPIC` 修改前缀。测试未主动注入故障或 rebalance，不代表故障情况下的端到端业务顺序保证。

## 环境变量

| 变量 | 默认值 | 用途 |
| --- | --- | --- |
| `KAFKA_BOOTSTRAP_SERVERS` | `localhost:9092` | Rust 客户端连接地址 |
| `KAFKA_TOPIC` | `confluent-rust-demo` | 脚本创建、客户端读写的 topic |
| `KAFKA_GROUP_ID` | `confluent-rust-example` | 消费组；demo 自动追加运行 ID |
| `KAFKA_MESSAGE_COUNT` | `10` | 生产/消费条数，必须为正整数 |
| `KAFKA_TIMEOUT_SECONDS` | `60` | 消费循环总等待时间，必须为正整数 |
| `KAFKA_WORKER_THREADS` | `4` | Tokio 工作线程池大小，必须为正整数 |
| `KAFKA_CLIENTS` | `3` | pool / broadcast 的独立消费者客户端数，必须为正整数 |
| `KAFKA_PARTITIONS` | `3` | 启动脚本新建 topic 的分区数；已有 topic 保留原分区数 |
| `KAFKA_PORT` | `9092` | Docker 映射到宿主机的端口 |
| `KAFKA_ADVERTISED_HOST` | `localhost` | 返回给宿主机客户端的 broker 主机名 |
| `KAFKA_CONTAINER_NAME` | `confluent-kafka-example` | Kafka 容器名 |
| `KAFKA_IMAGE` | `confluent-kafka-example:local` | 本地构建镜像名 |
| `KAFKA_NETWORK` | `confluent-kafka-example` | Docker 网络名 |
| `KAFKA_VOLUME` | `confluent-kafka-example-data` | 保存 Kafka 数据的 Docker 卷 |

修改端口时，需要同时修改客户端地址：

```bash
KAFKA_PORT=19092 ./start-kafka.sh
KAFKA_BOOTSTRAP_SERVERS=localhost:19092 ./run.sh demo
```

修改 topic 时，两侧使用同一个值：

```bash
KAFKA_TOPIC=orders ./start-kafka.sh
KAFKA_TOPIC=orders KAFKA_MESSAGE_COUNT=20 ./run.sh demo
```

宿主机通过 `localhost:9092` 连接，同一 Docker 网络内的客户端通过
`confluent-kafka:29092` 连接。默认端口仅绑定宿主机回环地址，使用明文连接，
配置用于本地开发。修改端口、镜像或其他容器参数前，先停止并删除旧容器；
启动脚本复用已有容器时会保留原配置。

## 查看日志与停止

```bash
docker logs -f confluent-kafka-example
./stop-kafka.sh
```

停止脚本删除容器并保留数据卷，下次启动继续使用原数据。
如需清空全部消息，在停止之后手动执行：

```bash
docker volume rm confluent-kafka-example-data
```
