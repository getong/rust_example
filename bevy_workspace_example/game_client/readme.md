# game_client local run

Agones 的 `172.21.x.x:<port>` 是 Docker/kind 内网地址，macOS 本机客户端不要直接连这个地址。

本地开发先启动 UDP proxy：

```bash
./scripts/start-game-udp-proxy.sh
```

再运行客户端：

```bash
GAME_SERVER_ADDR=127.0.0.1:30600 cargo run -p game_client --bin game_client
```

也可以用封装脚本：

```bash
./scripts/run-game-client-local.sh
```

完整部署流程见：

```bash
deploy/agones/README.md
```
