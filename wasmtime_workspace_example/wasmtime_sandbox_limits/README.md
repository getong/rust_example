# Wasmtime sandbox limits example

这个例子参考 `ironclaw` 里的 Wasmtime sandbox 做法，集中演示四类限制：

- `fuel`: 给 `Store` 注入固定燃料。Wasm 每执行一段指令都会消耗 fuel，耗尽后确定性 trap，适合限制 CPU 指令预算。
- `epoch interruption`: 给 `Engine` 开启 epoch 检查，并由宿主线程定时 `increment_epoch()`。当 `Store` 的 epoch deadline 到达时，运行中的 Wasm 会被中断，适合作为 wall-clock timeout 后备保护。
- `memory limiter`: 通过 `Store::limiter` 挂载 `ResourceLimiter`，在 `memory.grow` 或内存创建时决定是否允许增长，防止 guest 占用过多线性内存。
- `table limiter`: 同样通过 `ResourceLimiter::table_growing` 限制 `table.grow`，防止 guest 创建过大的函数/引用表。
- `instance limiter`: 通过 `ResourceLimiter::instances` 限制同一个 `Store` 内可创建的实例数量，防止大量实例耗尽宿主资源。

运行：

```sh
cargo run -p wasmtime_sandbox_limits
```

输出会展示：

1. 正常函数调用成功，并打印消耗/剩余 fuel。
2. 无限循环在 fuel 耗尽后 trap。
3. 不启用 fuel 时，同样的无限循环会被 epoch deadline 截断。
4. `memory.grow` 第一次成功，第二次超过 2 页限制后返回 `-1`。
5. `table.grow` 第一次成功，第二次超过 3 个元素限制后返回 `-1`。
6. 同一个 `Store` 中第二次实例化超过 instance 限制而失败。

和 `ironclaw` 的对应关系：

- `Config::consume_fuel(true)` 对应运行时 CPU metering。
- `store.set_fuel(limit)` 对应每次执行前注入 fuel budget。
- `Config::epoch_interruption(true)`、后台 ticker 和 `store.set_epoch_deadline(...)` 对应 timeout backstop。
- `store.limiter(|data| &mut data.limiter)` 对应把 memory/table/instance 限制绑定到 store state。
