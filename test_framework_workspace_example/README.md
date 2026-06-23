# Rust testing framework workspace

这个 workspace 为每个框架放了一个独立 Rust project，用最小但可运行的代码说明它们的作用和典型功能。

| Project | Framework | 作用 | 运行方式 |
| --- | --- | --- | --- |
| `proptest_demo` | `proptest = "1.11"` | 属性测试。自动生成大量输入，失败时会 shrink 到更小反例。 | `cargo test -p proptest_demo` |
| `quickcheck_demo` | `quickcheck = "1.1"` | 轻量级属性测试。用函数签名驱动随机输入生成。 | `cargo test -p quickcheck_demo` |
| `criterion_demo` | `criterion = { version = "0.8", features = ["html_reports"] }` | 统计型 benchmark。提供稳定计时、回归比较和 HTML 报告。 | `cargo bench -p criterion_demo` |
| `mockall_demo` | `mockall = "0.14"` | mock trait 依赖。适合隔离外部服务、数据库、仓储等边界。 | `cargo test -p mockall_demo` |
| `wiremock_demo` | `wiremock = "0.6"` | HTTP mock server。用于测试 HTTP client 的请求匹配和响应处理。 | `cargo test -p wiremock_demo` |
| `fake_demo` | `fake = { version = "5.1", features = ["derive"] }` | 生成假数据。适合构造测试 fixture、示例数据和本地开发数据。 | `cargo test -p fake_demo` |

也可以一次验证全部测试：

```sh
cargo test --workspace
```

benchmark 需要单独执行：

```sh
cargo bench -p criterion_demo
```
