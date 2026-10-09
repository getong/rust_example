# 抗量子协作通道 v2

参考 `aws_lc_rs_workspace_example/encrypt_file` 的算法选型，Rust client/server 共用 `protocol/src/crypto.rs`，密码操作由 `aws-lc-rs 1.18.1` 完成。网页通过固定版本 `@noble/post-quantum 0.5.4` 和 WebCrypto 与 AWS-LC 互通，不是把 AES 或 RSA 单独称为抗量子方案。

## 算法与消息

- ML-KEM-1024：每次 API 操作生成新的客户端临时密钥；服务端封装得到共享密钥。
- ML-DSA-87：每次握手新生成服务端签名密钥，并在回复中附带公钥；不持久化或预配密钥。
- HKDF-SHA256：以 `SHA256(transcript)` 为 salt、KEM 共享密钥为输入，分别用 `c2s` / `s2c` 标签派生 32 字节密钥。
- AES-256-GCM：请求和响应使用不同密钥，AAD 为上述 transcript hash；每个方向只发送一个消息。

固定 suite 字符串：

```text
topcoat-pq-v2:ML-KEM-1024:ML-DSA-87:HKDF-SHA256:AES-256-GCM
```

1. `POST /pq/handshake`：`{suite, public_key}`，public_key 是 1568 字节 ML-KEM 公钥的小写 hex。
2. 返回 `{suite, session, kem, public_key, signature}`：32 字节随机会话标识、1568 字节 KEM 密文、2592 字节临时签名公钥、4627 字节 ML-DSA 签名，均为小写 hex。
3. 被签名 transcript = suite 的 UTF-8 字节 + 客户端公钥原始字节 + session 原始字节 + KEM 密文原始字节 + 服务端签名公钥原始字节。所有字段长度固定，版本和算法也参与签名。客户端先验签再解封装。
4. `POST /pq/exchange`：`{session, ciphertext}`。ciphertext 为 AES-GCM 密文及 16 字节 tag 的小写 hex。加密内容是 `{method,path,body,form:false}` 的 UTF-8 JSON；`body` 为 JSON 字符串或空字符串。
5. 返回同形 envelope，加密内容为 `{status,body}`。业务错误的状态码和内容也在加密层内；外层 HTTP 成功不代表业务成功。

GCM nonce 固定为 12 字节零，**仅因每次握手生成全新密钥，而且每个方向只使用一次**。Rust 用消耗所有权的接口防止同一 channel 重复加密，服务端在业务执行前原子删除会话。不能在此协议上直接增加循环发包或重试；若要复用会话，必须另行设计序列号、唯一 nonce、防重放窗口并升级协议版本。

## 身份、信任与边界

客户端不再预设公钥；服务端不再读取持久私钥。每次业务操作开始时执行应用握手：客户端生成 ML-KEM 密钥，服务端生成 ML-DSA 密钥并返回临时公钥。HTTP keep-alive 是否复用 TCP 连接不影响密钥刷新。协议版本升级为 v2，旧客户端会拒绝，双方需一起更新。

动态公钥通过握手原样传回，不能自行证明服务器身份。远程桌面连接只允许正常证书校验的 HTTPS；网页同样要求可信 HTTPS 加载页面、脚本与握手。本机开发允许 localhost/127.0.0.1/::1 HTTP，但无法抵抗本机中间人替换整个握手。临时 ML-DSA 签名校验验证消息一致性，不应被描述为独立的身份认证。普通 TLS 的身份认证也不自动具有抗量子安全性；抗量子密钥协商保护业务内容不等于整个部署具备完全抗量子身份保证。

不缓存服务端签名公钥，也不再使用 `/pq/public-key`、`--keygen`、`TOPCOAT_SIGNING_KEY`、`TOPCOAT_SERVER_PUBLIC_KEY`。验签或 AEAD 失败则终止，不降级或自动重试业务写入。

该示例提供传输保密、基于 HTTPS 的服务端身份验证和消息完整性，没有添加用户登录、客户端身份认证或授权。任何能访问服务的人仍可建立自己的会话读取/修改共享数据。服务端解密后内存中保存业务数据，没有磁盘数据加密；握手私钥只驻留内存，不写入文件。

算法采用现有抗量子标准，但这是一份应用协议示例，未经独立密码协议审计，不承诺对所有未来攻击的绝对安全或整个系统的 FIPS 认证。

## 生命周期与限制

- 会话最多 1024 个，60 秒过期；握手时清理过期项，容量满时返回 429。单进程内处理会话；多实例部署需粘性路由或另行设计共享状态。
- 会话标识不承担客户端认证。知道标识的攻击者可以提交坏包令该会话失效（拒绝服务），不能读取或伪造其业务消息。
- 明文消息上限 4 MiB；HTTP 外层 envelope 有独立大小限制；握手 HTTP body 上限 8 KiB。原业务输入限制仍有效。
- 不自动重试修改。丢失响应时写入可能已完成，应刷新确认。
- 重启服务丢弃业务数据和临时会话；新连接自动生成新密钥，无需更新公钥文件或重新加载网页以刷新公钥。
- 临时服务端签名私钥在握手完成后释放；客户端 KEM 私钥在密钥派生后释放；业务会话密钥只用于一个请求/响应。

## 验证

协议测试覆盖双向加解密、错误签名公钥、握手替换、错误 suite、密文篡改及请求反射。服务端测试覆盖每次握手不同公钥、过期、体积限制、并发重放只执行一次、明文 API 拒绝及加密业务错误。异步 HTTP 测试在同一线程运行双方，验证网络等待不会阻塞服务。

`scripts/smoke.py` 使用随机端口和动态握手，执行 Rust → TypeScript/WebCrypto → Rust 的业务协作。网页项目 `bun run test:e2e` 验证真实浏览器发布、桌面同步、草稿保留和篡改响应后的恢复。

参考：[AWS-LC KEM API](https://docs.rs/aws-lc-rs/latest/aws_lc_rs/kem/)、[AWS-LC signature API](https://docs.rs/aws-lc-rs/latest/aws_lc_rs/signature/index.html)、[noble-post-quantum](https://github.com/paulmillr/noble-post-quantum)。
