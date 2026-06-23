# fake_demo

`fake` 用于生成测试数据和示例数据。它内置姓名、邮箱、地址、公司等常见 faker，也支持 derive 为结构体自动填充字段。

本项目展示：

- 使用 `#[derive(Dummy)]`。
- 为字段指定 faker，例如 `Name()`、`FreeEmail()`、`Username()`。
- 构造单个或多个测试用户 fixture。

运行：

```sh
cargo test -p fake_demo
```
