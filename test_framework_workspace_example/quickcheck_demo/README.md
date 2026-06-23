# quickcheck_demo

`quickcheck` 是更轻量的属性测试框架。它通过测试函数参数类型生成随机输入，写法接近普通单元测试。

本项目展示：

- 用 `#[quickcheck]` 标记属性测试。
- 自动生成 `Vec<u8>` 输入。
- 验证 checksum 和过滤逻辑的不变量。

运行：

```sh
cargo test -p quickcheck_demo
```
