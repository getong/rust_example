# RocketMQ consumer demo

参考 `rocketmq-rust/rocketmq-client/examples/quickstart/consumer.rs` 和集群消费示例，使用 `rocketmq-client-rust 1.0.0` 的 `DefaultMQPushConsumer`。

## 运行

1. 启动 RocketMQ NameServer 和 Broker，确保 Broker 注册到 NameServer，且它公布的地址和端口可从本机访问。
2. 确保 `TopicTest` 已创建，或 Broker 已允许自动创建主题并由 producer 完成首次发送。
3. 在当前目录启动 consumer：

   ```bash
   cargo run -p consumer
   ```

4. 在另一个终端进入同一工作区，运行现有 producer：

   ```bash
   cargo run -p producer
   ```

consumer 会输出消息 ID、Topic、Tag、队列、消费位点、重试次数及正文。现有 producer 默认发送到 `TopicTest`。按 Ctrl+C 退出；Unix 下也支持 SIGTERM。

## 配置

| 环境变量 | 默认值 | 说明 |
| --- | --- | --- |
| `NAMESRV_ADDR` | `127.0.0.1:9876` | 多个 NameServer 地址使用分号分隔 |
| `CONSUMER_GROUP` | `example_consumer_group` | 同组实例分担消费，订阅关系应一致 |
| `TOPIC` | `TopicTest` | 订阅主题，需与 producer 一致 |
| `TAGS` | `*` | 所有 Tag；也可使用 `TagA \|\| TagB` |

```bash
NAMESRV_ADDR='127.0.0.1:9876' \
CONSUMER_GROUP='example_consumer_group' \
TOPIC='TopicTest' \
TAGS='*' \
cargo run -p consumer
```

未设置的变量使用默认值；空值或非 Unicode 配置会报错。现有 producer 未设置 Tag，联调时请保留 `TAGS='*'`。

## 消费语义与业务扩展

- 使用默认的 **Clustering 集群模式**，注册并发监听器，消费线程数为 1–10，每次回调最多处理 1 条消息。不保证消息顺序。
- 保留 SDK 默认的起始位点策略。已有消费组从已提交位点继续消费；新组默认从末尾开始（特殊主题除外），因此建议先启动 consumer，再发送新消息。重启不会自动重放已确认的消息。
- `Listener::consume_message` 是业务处理入口。当前仅打印消息并返回 `ConsumeSuccess`；二进制正文以有损 UTF-8 展示，原始数据仍可通过 `get_body()` 读取。
- 业务处理成功后才能返回 `ConsumeSuccess`。处理失败时改为返回 `Ok(ConsumeConcurrentlyStatus::ReconsumeLater)`，交由 Broker 重试。消息可能重复投递，业务应保证幂等；不要在这个同步回调中直接执行长时间阻塞操作。
- 退出时按 consumer → client runtime → runtime owner → telemetry 的顺序清理。订阅、启动或等待信号失败也会执行清理；关闭报告异常会使程序以错误状态退出。

## 检查

```bash
cargo fmt -p consumer -- --check
cargo check -p consumer --locked
cargo clippy -p consumer --locked -- -D warnings
```

编译检查不需要 Broker；实际收发联调需要运行中的 NameServer 和 Broker。
