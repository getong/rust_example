# HelixDB Rust example

启动 Docker Desktop，然后执行：

```sh
./start-helix.sh
cargo run
```

脚本使用本机已下载的 `ghcr.io/helixdb/helixdb:v0.0.5` 镜像，
后台启动 `helix-helix_db_example-v3` 容器，并等待 `/readyz` 就绪检查成功。
重复运行会复用已有容器；镜像缺失时会提示下载，不会自动拉取。

当前 `helix-db = "3.0.0"` SDK 连接 `http://127.0.0.1:6969`，无需 API key。
`cargo run` 执行创建、查询、更新、删除，最后查询应返回空数组。
客户端请求已迁移到 SDK 3.0 的 `QueryRequest` 和 `client.query(request)` API。
旧版 `enterprise-dev` 镜像的 `/v1/query` 不适用于当前 SDK。
若 6969 端口被其他容器或服务占用，请先停止该服务。

数据持久化在 Docker volume `helix-helix_db_example-v3-data` 中，
停止和重启容器不会清空数据。

```sh
# 查看日志
docker logs --tail 100 helix-helix_db_example-v3
# 停止服务
docker stop helix-helix_db_example-v3
# 再次启动
./start-helix.sh
```
