# 用 Rust + Axum + JWT 理解 Unreal Session

你的类比有一半是对的：**可以用 Axum HTTP 服务 + JWT 实现一个教学用的 Session 后端，但 Unreal Session 不等于 HTTP 服务 + JWT 鉴权。**

```text
JWT / 登录身份：你是谁？凭证是否有效？
Session / 对局会话：有哪些对局？能否加入？谁在里面？满了吗？开局了吗？
游戏网络连接：连接哪个服务器？同步哪些 Actor？怎么同步移动、RPC 和战斗？
```

本项目重点模拟第二层，并用第一层保护它。第三层只模拟“连接准入检查”，**没有启动 UE 游戏服，没有 UDP、Actor Replication、RPC 或真实 ClientTravel**。这里的 Session 不是浏览器登录 Session / Cookie，也不是 UE 的完整网络协议。

## 运行

需要 Rust（支持 edition 2024）、Cargo；演示脚本还需要 Bash、curl、jq。

终端一，在当前目录运行：

```bash
export JWT_SECRET="$(openssl rand -hex 32)"
cargo run
```

终端二：

```bash
bash scripts/demo.sh
```

脚本会输出响应和预期 HTTP 状态，隐藏 token，走完创建 → 搜索 → 加入 → 模拟连接 → 退出/重新加入 → 开局 → 结束 → 销毁。可以使用 `BASE_URL=http://127.0.0.1:3001 bash scripts/demo.sh` 指向其他监听地址。

环境变量：

| 变量 | 默认值 | 用途 |
| --- | --- | --- |
| `JWT_SECRET` | 无，必须设置，至少 32 字节 | HS256 签名密钥；长度检查不代表随机性检查 |
| `BIND_ADDR` | `127.0.0.1:3000` | Axum 监听地址 |
| `GAME_SERVER_ADDR` | `127.0.0.1:7777` | 返回给客户端的示例连接地址，不会监听这个端口 |

教学账号为 `alice`、`bob`、`charlie`，密码均为 `demo-password`。这是**硬编码演示账号**，不是生产登录方案。不要直接将这个服务暴露到公网。

## 对照 Unreal 的概念

这里主要借用传统 Online Subsystem 的 `IOnlineSession` 方法名进行类比，不是对某个 UE 版本或 EOS/Steam 协议的实现；不同 Online Services / Subsystem 提供者的行为和能力可能不同。

| Unreal 概念 | 本项目模拟 | 实际解决的问题 |
| --- | --- | --- |
| 平台登录 / Identity | `POST /auth/login` 发 API JWT | 确认玩家身份；不自动获得房间成员资格 |
| `CreateSession` | `POST /sessions` | 发布对局记录、指定地图/模式/容量，房主占一个位置 |
| `FOnlineSessionSettings` / `UpdateSession` | `PUT /sessions/{id}/settings` | 更新名称、地图、模式、容量、是否可搜索、是否允许中途加入 |
| `FindSessions` / 搜索结果 | `GET /sessions?map=Harbor&mode=Deathmatch` | 从已公开、可加入、未满员的对局中筛选 |
| `JoinSession` / 成员注册的简化合并 | `POST /sessions/{id}/join` | 原子检查名额和规则，记录成员关系 |
| `GetResolvedConnectString` | 创建/加入响应中的 `connect_string` | 获得游戏端点；不是已经连上游戏服 |
| 客户端 Travel / 游戏服准入的类比 | `POST /game/connect` | 校验游戏票据与当前成员资格；**不是 UE 原生 API** |
| `StartSession` | `POST /sessions/{id}/start` | 把对局标记为进行中；不等于加载地图或启动游戏进程 |
| `EndSession` | `POST /sessions/{id}/end` | 标记本轮结束；记录仍然存在 |
| `DestroySession` | `DELETE /sessions/{id}` | 删除会话记录；不等于杀掉游戏服务器进程 |

本例为便于观察，将服务端的权威对局记录和玩家成员关系放在同一个内存表中。真实 UE 还存在每个客户端/服务端的本地命名 Session、异步完成委托，以及提供者侧的会话表示；不要把本例的 `DELETE` 理解为所有平台上客户端退出的唯一方式。这里的 `leave` 仅表示移除成员，不与 `DestroySession` 一一等价。

## 一次完整调用

下面的命令和脚本一样，都使用 curl + jq。可以对一个新启动的服务单独执行。

```bash
ALICE=$(curl -fsS http://127.0.0.1:3000/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"demo-password"}' | jq -r .access_token)

ROOM=$(curl -fsS http://127.0.0.1:3000/sessions \
  -H "Authorization: Bearer $ALICE" \
  -H 'Content-Type: application/json' \
  -d '{"name":"Alice room","map":"Harbor","mode":"Deathmatch","max_players":2,"advertised":true,"allow_join_in_progress":false}')
SESSION_ID=$(printf '%s' "$ROOM" | jq -r .session.id)

BOB=$(curl -fsS http://127.0.0.1:3000/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"bob","password":"demo-password"}' | jq -r .access_token)

curl -fsS 'http://127.0.0.1:3000/sessions?map=Harbor&mode=Deathmatch' \
  -H "Authorization: Bearer $BOB" | jq

JOIN=$(curl -fsS -X POST "http://127.0.0.1:3000/sessions/$SESSION_ID/join" \
  -H "Authorization: Bearer $BOB")
printf '%s' "$JOIN" | jq 'del(.join_ticket)'
TICKET=$(printf '%s' "$JOIN" | jq -r .join_ticket)

jq -nc --arg ticket "$TICKET" '{ticket:$ticket}' | \
  curl -fsS http://127.0.0.1:3000/game/connect \
    -H 'Content-Type: application/json' -d @- | jq
```

创建和加入都返回如下结构（房主也可以拿到自己的连接票据）：

```json
{
  "session": {
    "id": "<session uuid>",
    "host_id": "alice",
    "state": "waiting",
    "settings": {
      "name": "Alice room",
      "map": "Harbor",
      "mode": "Deathmatch",
      "max_players": 2,
      "advertised": true,
      "allow_join_in_progress": false
    },
    "members": [{"player_id":"alice"}, {"player_id":"bob"}]
  },
  "connect_string": "127.0.0.1:7777",
  "travel_url_example": "127.0.0.1:7777?Ticket=<join_ticket>",
  "join_ticket": "<signed JWT>",
  "expires_in": 120
}
```

`travel_url_example` 是伪示例：如果在 UE 中使用自定义 `Ticket` URL 参数，必须自行实现解析和服务端验证；UE 不会自动理解这个 JWT。实际凭证不应记录进日志。

## 为什么单靠 JWT 不够？

两个 JWT 用相同教学密钥签发，但有不同的用途和有效期：

| 凭证 | 有效期 | 内容 / 使用方式 |
| --- | --- | --- |
| API access token | 3600 秒 | `sub=玩家`，`aud=session-api`；Session API 的 `Authorization: Bearer ...` |
| join ticket | 120 秒 | `sub=玩家`，`aud=game-server`，另含 `session_id`、`membership_id`；提交给 `/game/connect` |

两者均验证 HS256 签名、过期时间、issuer 和 audience，不能互换。`membership_id` 是每次新加入时生成的随机 UUID，不会出现在公开 Session JSON 中。连接时除了验 JWT，还查服务端的当前成员关系和会话状态：

```text
Bob 的 API JWT 合法
  → 房间满员？409，不能加入
  → Bob 不是房主？403，不能开局/改设置/销毁
  → 已开局且禁止中途加入？409，不能新加入

Bob 已取得 join ticket
  → 没退出且对局未结束？模拟准入通过
  → 已退出？403，即使票据还没过期也无效
  → 退出又加入？旧 membership_id 不匹配，旧票据仍然无效
  → 对局结束？409；对局销毁？404
```

这就是 **身份认证（authentication）≠ 对局授权（authorization）≠ 游戏连接**。本例的 ticket 是短期可重复使用的 Bearer 凭证，**不是一次性票据**；退出/结束仅影响之后的准入检查，没有真实连接可断开。API access token 本身没有登出撤销机制。

## 生命周期与规则

```text
Create → Waiting ──Start──→ InProgress ──End──→ Ended
            │                  │                │
            └────Destroy───────┴────Destroy─────┘ → 删除
```

- 这是单轮教学状态机：不支持 `Ended → Waiting`、重赛、房主迁移；真实 UE/提供者的状态与可重入规则更复杂。
- 只有房主能更新设置、开局、结束、销毁。设置只能在 `Waiting` 修改，不能把容量调低到当前人数以下。
- 房主创建时自动注册；`max_players` 包含房主。重复加入不重复占位，会刷新当前成员的短期票据。
- `allow_join_in_progress=false` 只禁止**新成员**中途加入；已经注册的成员可在结束前刷新票据/再次模拟连接。
- `advertised=false` 表示不在搜索中展示，**不是私密房间/邀请鉴权**：知道 ID 的已登录玩家仍可加入。非成员直接读取该隐藏房间详情会得到 404。
- `End` 保留记录但从搜索中消失；本例还禁止结束后的连接准入。这是演示策略，不代表 Unreal 的 `EndSession` 会自动踢掉游戏网络连接。
- 非房主退出释放位置；房主不能直接退出，必须销毁会话。不在线不等于退出，未实现心跳超时回收。
- 同一进程内用 `Arc<RwLock<HashMap<Uuid, Session>>>` 保护会话，检查人数和插入成员在同一写锁内，避免并发超卖。多个服务实例间不共享这把锁。

## HTTP 接口

除 `/health`、`/auth/login`、`/game/connect` 外都要求 API Bearer token。

| 方法 | 路径 | 请求 / 说明 |
| --- | --- | --- |
| GET | `/health` | 文本 `ok` |
| POST | `/auth/login` | `{ "username": "alice", "password": "demo-password" }` |
| POST | `/sessions` | 完整 settings JSON；返回 201 + Session 和票据 |
| GET | `/sessions` | 可选 `map`、`mode` 精确匹配；不返回满员/隐藏/不可加入的会话 |
| GET | `/sessions/{id}` | 查看详情（隐藏房间仅限成员） |
| PUT | `/sessions/{id}/settings` | 完整替换 settings JSON，不是局部 PATCH |
| POST | `/sessions/{id}/join` | 无 body；返回成员关系和连接信息 |
| POST | `/sessions/{id}/leave` | 无 body；非房主退出，重复退出幂等 |
| POST | `/sessions/{id}/start` | 无 body；Waiting → InProgress |
| POST | `/sessions/{id}/end` | 无 body；InProgress → Ended |
| DELETE | `/sessions/{id}` | 删除后返回 204 |
| POST | `/game/connect` | `{ "ticket": "<join_ticket>" }`；仅模拟游戏服准入 |

业务错误响应为 `{"error":"..."}`：400 参数规则错误，401 凭证错误，403 身份合法但无权限，404 会话不存在/不可见，409 满员/状态冲突。Axum 对格式错误的 JSON、UUID、查询参数等仍使用框架默认的 4xx rejection 格式。

## 代码导航与边界

- `src/main.rs`：Axum 路由、共享状态、环境变量、启动/退出。
- `src/auth.rs`：演示登录、JWT 签发验证、Bearer 中间件。
- `src/sessions.rs`：会话设置、搜索、成员关系、生命周期、模拟准入。
- `src/error.rs`：业务错误到 HTTP 的映射。
- `scripts/demo.sh`：可执行调用示例，包含预期失败分支和结果校验。

所有 Session 都在内存中，重启即丢失；没有持久化、分页、限流、账号注册/密码哈希、token 撤销、邀请、匹配队列、延迟探测、NAT 穿透、连接保活或专服分配。允许一个玩家参加多个会话。所有房间共享一个配置的示例游戏端点，不代表真实的多房间服务器拓扑。

生产中应使用 TLS、真正的身份服务、强随机密钥和轮换机制、不同服务的凭证信任边界，以及数据库事务/原子容量更新；真实游戏服应独立验证授权，不应把本例共享 HS256 密钥的单进程方式照搬成跨信任域方案。

## 官方参考

- [Epic：Online Subsystem Session Interface](https://dev.epicgames.com/documentation/en-us/unreal-engine/online-subsystem-session-interface-in-unreal-engine)
- [Epic：Travelling in Multiplayer](https://dev.epicgames.com/documentation/en-us/unreal-engine/travelling-in-multiplayer-in-unreal-engine)
- [Axum：共享状态与中间件](https://docs.rs/axum/0.8/axum/)
- [jsonwebtoken：签发与验证](https://docs.rs/jsonwebtoken/10/jsonwebtoken/)

**一句话：Axum 是通信入口，JWT 是身份证，Session 是对局登记簿和规则管理器，而真正的游戏连接是另一套网络系统。**
