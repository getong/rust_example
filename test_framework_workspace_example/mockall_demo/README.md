# mockall_demo

`mockall` 用于为 trait 生成 mock，常见于隔离数据库、HTTP 客户端、消息队列、文件系统等外部依赖。

本项目展示：

- 生产代码依赖 `BalanceStore` trait。
- 测试里用 `mock!` 生成 `MockStore`。
- 用 `expect_*`、`with`、`times`、`return_const` 约束调用参数、次数和返回值。

运行：

```sh
cargo test -p mockall_demo
```
