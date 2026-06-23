# wiremock_demo

`wiremock` 是 HTTP mock server。它让测试启动一个本地服务器，声明请求匹配条件，再返回指定响应。

本项目展示：

- 启动 `MockServer`。
- 用 `method` 和 `path` 匹配 HTTP 请求。
- 用 `ResponseTemplate` 返回 JSON。
- 验证客户端侧路径和 JSON 解析逻辑。

运行：

```sh
cargo test -p wiremock_demo
```
