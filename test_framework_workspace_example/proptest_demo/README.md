# proptest_demo

`proptest` 是属性测试框架。它不只检查几个手写 case，而是按照策略生成大量输入，再验证某个性质始终成立。

本项目展示：

- 用 `proptest::collection::vec` 自动生成 `Vec<i32>`。
- 用正则策略生成字符串输入。
- 用 `prop_assert!` / `prop_assert_eq!` 表达不变量。
- 失败时自动 shrink，帮助定位最小反例。

运行：

```sh
cargo test -p proptest_demo
```
