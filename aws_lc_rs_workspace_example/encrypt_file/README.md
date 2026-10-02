# ML-KEM 文件加密示例

使用 ML-KEM-1024、HKDF-SHA256、AES-256-GCM 加密文件，并用 ML-DSA-87 验证发送方身份。默认写入带签名的文件格式 v2；旧 v1 文件只能显式选择兼容模式读取。这是自定义格式的学习示例，不是独立审计或经过 FIPS 认证的产品。

## 使用

```sh
# 接收方生成加密密钥，默认必须设置非空密码，并确认一次
cargo run --release --bin decrypt -- --keygen recipient-v2.public recipient-v2.private

# 发送方独立生成签名密钥，设置其自己的非空密码
cargo run --release --bin decrypt -- --sign-keygen sender.public sender.private

# 加密：依次输入接收方公钥密码和发送方签名私钥密码
cargo run --release --bin encrypt -- --sign-key sender.private recipient-v2.public input.txt encrypted-v2.bin

# 解密：先验证可信发送方的签名，再输入接收方私钥密码
cargo run --release --bin decrypt -- --verify-key sender.public recipient-v2.private encrypted-v2.bin recovered-v2.txt
```

`sender.public` 必须预先经可信渠道交付并核对指纹，不能直接信任与密文一起从可被替换的 URL 下载的公钥。签名私钥由发送方独立保存，接收方私钥由接收方独立保存；两者不能混用，签名私钥及其密码不应随接收方公钥分发。

根据当前使用约定，接收方公钥仍默认用与接收方私钥相同的密码保护，因此加密时仍需解锁公钥。公钥加密不是发送方身份认证，也不是增强 ML-KEM 的加密强度。若希望公开分发裸公钥，可在 `--keygen` 后显式加 `--public-unprotected`，此时仍必须给私钥设密码。签名公钥始终裸存，以便先验证签名。

同一密码保护接收方公私钥意味着，把公钥密码给发送方也会让其知道接收方私钥的密码。若私钥文件随后泄漏，这个密码不再提供双方隔离；面向其他发送方分发时应考虑 `--public-unprotected`，仅保护私钥。

只有显式添加 `--no-password` 才会生成未加密私钥，CLI 会警告并输出公钥指纹。仅按 Enter 会报错；EOF、密码不一致也不会生成密钥。终端由 `inquire::Password` 显示 `*` 掩码；管道按行读取，不回显，保留首尾空格，去掉 LF/CRLF。密码上限为 1024 字节（不含行尾）。

所有输出使用排他创建，不覆盖已有文件。示例使用新文件名；不要删除旧私钥，旧密文仍需要原来的私钥。磁盘写入失败可能留下不完整文件；生成密钥先写私钥，公钥写入失败时已写入的私钥会保留。

## 公钥指纹核对

```sh
cargo run --release --bin decrypt -- --fingerprint kem recipient-v2.public
cargo run --release --bin decrypt -- --fingerprint sign sender.public
```

指纹是解锁后规范原始公钥字节的 SHA-256（64 个十六进制字符），不是加密密钥文件的 hash。生成时也会输出指纹。通过面对面、已认证消息渠道等带外方式核对完整指纹，再保存为本地可信公钥；不要从同一个不可信下载地址同时获取公钥和“可信指纹”。换钥时重新核对，不自动接受文件头声明的发送方身份。

## 替换攻击与签名的边界

仅有公钥加密时，拿到公钥的人可以生成一份全新的合法密文；AES-GCM 的完整性校验不能证明是谁发的。这也是旧 v1 文件的限制。

v2 的头部记录发送方和接收方公钥指纹、原文长度；整头作为 AES-GCM AAD，并绑定进 HKDF info。ML-DSA-87 签名覆盖协议专用域、整个头部，以及密文（包含 GCM tag）的 SHA-512 摘要。签名附在文件末尾，由外部可信发送方公钥验证，通过后才解锁接收方私钥、执行 KEM/AEAD 并创建输出。

签名不阻止可信发送方自己重发旧文件，也不防重放、回滚、拒绝服务或签名私钥泄漏。需要防回滚时，必须在应用层验证预期版本/时间/文件身份。任意拿到发送方私钥的人仍可伪造来源。

## 格式、长度泄漏与兼容性

文件 v2 的完整头部为 1694 字节：

| 字段 | 字节数 |
|---|---:|
| 魔数 `ALCFENC\0`、版本 `2`、套件 `1` | 10 |
| HKDF 随机盐 | 32 |
| ML-KEM-1024 封装密文 | 1568 |
| GCM nonce | 12 |
| 发送方公钥 SHA-256 指纹 | 32 |
| 接收方公钥 SHA-256 指纹 | 32 |
| 明文长度，小端 u64 | 8 |

随后为 AES-GCM 密文、16 字节 GCM tag、4627 字节 ML-DSA-87 签名。总长度为明文长度 + **6337**，精确暴露明文长度，并公开双方的指纹；本实现不填充、不隐藏元数据。ML-DSA 签的是自定义 transcript，不宣称是 HashML-DSA 模式：

```text
"encrypt_file/v2/signature/ML-DSA-87/SHA-512\0" || header || SHA512(ciphertext || GCM_tag)
```

v2 的 HKDF info 为 `encrypt_file/v2/ML-KEM-1024/HKDF-SHA256/AES-256-GCM/ML-DSA-87` 后连接整个头部，包含 KEM 密文。每次加密重新封装，并生成随机盐和 nonce；不声称数学上绝无随机碰撞。不增加额外 HMAC，也不作整个组合达到某一 NIST 类别的合规承诺。

旧文件 v1 的头部保持 1622 字节，总长度为明文长度 + 1638；旧 HKDF info `encrypt_file/v1/ML-KEM-1024/HKDF-SHA256/AES-256-GCM` 完全不变。读取必须明确选择无签名兼容模式：

```sh
cargo run --release --bin decrypt -- --allow-unsigned-legacy recipient.private encrypted.bin recovered-legacy.txt
```

可信验签模式拒绝 v1，兼容模式拒绝 v2；不会遇到签名错误就降级。旧裸密钥、Argon2id v2 密钥、PBKDF2 v1 私钥仍可读取。旧裸公钥不含密码信息，不会因程序升级自动受到密码保护；旧密钥可直接参与新的签名文件流程。只有新建签名密钥是新流程必须补充的步骤。

密码保护密钥容器仍为 `ALCFKEY\0`、版本 `2`、套件 `1`、类型 u8，然后是三个小端 u32 Argon2 参数、32 字节盐、12 字节 nonce，共 67 字节头部。类型：0=KEM 公钥、1=KEM 私钥、2=签名公钥、3=签名私钥。原始长度分别为 1568、3168、2592、4896 字节；非空密码时整个头部作为 AAD。签名公钥写出时不加密码。旧 v1 私钥的 600,000 次 PBKDF2-HMAC-SHA256 迭代保持不变。

新密钥使用 Argon2id v0x13，64 MiB / t=3 / p=1；读入仅接受内存 64–256 MiB、t=3–10、p=1–4，参数在分配内存前检查。该配置不是 RFC 9106 第一推荐档的逐字实现；RFC 的 64 MiB 第二档示例使用 p=4，详情见 [RFC 9106 §4](https://www.rfc-editor.org/rfc/rfc9106.html#section-4)。

## 库接口和文件保护

- `format`：不分配内存的格式/长度校验；`kdf`：密码派生与版本化 HKDF。
- `keys`：密钥容器和指纹；`signing`：签名及显式信任策略；`crypto`：加解密流程。
- `file`：有界读取、排他写入及同步；`platform`：Windows ACL；`src/cli`：终端密码输入。
- `PasswordSource` 由调用方提供，支持 `FixedPassword`、`NoPassword` 或自定义实现。库没有 stdin/TTY/inquire 调用。
- `Error` 为结构化枚举，可匹配 `KeyUnlockFailed`、`SignatureInvalid`、`AuthenticationFailed`、`UnsupportedVersion`、`KdfParameters` 等；密码错误与密钥被篡改统一报 `KeyUnlockFailed`，不承诺区分这两种情况。

文件读取上限为 64 MiB 明文，拒绝非普通文件以及读取时长度变化；同长度并发改写不能仅通过长度检测发现。密码、私钥、派生字节、明文和 Argon2 工作内存用 `Zeroizing` 擦除；不覆盖 inquire 内部缓冲、交换区、崩溃转储和调用方自己保留的副本，也不提供 mlock。

Unix 新文件权限为 0600，并同步文件及父目录；Windows 使用仅 Owner Rights 可访问且禁止继承的 DACL，需要支持 ACL 的文件系统。签名与 AEAD 均验证成功后才创建明文输出；尚未实现 GB 级流式格式。

## 测试与性能

```sh
cargo fmt -p encrypt_file -- --check
cargo clippy -p encrypt_file --all-targets --all-features -- -D warnings
cargo test -p encrypt_file --release --lib --tests
cargo bench
cargo bench --features perf-trace
cargo bench --bench cli -- protected/encrypt/4B --samples 5
```

基准仍为原生 Rust，14 个场景各预热一次、默认采样三次，输出中位数/min/max。加密包含公钥解锁、签名私钥解锁和签名；解密包含验签及私钥解锁，签名密钥生成作为不计时的准备阶段。小文件密码开销与旧无签名版本不能直接混比。计时包含进程启动、磁盘同步和擦除，排除输入等待、准备、校验、清理。临时数据不影响用户文件；不采集 CPU 时间/峰值 RSS。`perf-trace` 的父子阶段有包含关系，不能直接相加。

`tests/fixtures` 含固定旧 v1 文件和 NIST 官方 ACVP ML-KEM-1024 已知答案，包含隐式拒绝测试，来源与哈希见其中 README。它们的私钥是公开测试材料，不能用于实际数据。此类回归测试不等于算法或产品认证。

fuzz 目标和运行方法见 [fuzz/README.md](fuzz/README.md)。仓库根 `.github/workflows/encrypt-file.yml` 配置 macOS/Windows 的 fmt、Clippy、回归测试；只编译 benchmark，不在 CI 执行基准。Windows 测试包含 DACL 检查，本地 macOS 成功不能代替 Windows runner 的结果。

评审逐项落实和修正说明见 [docs/crypto-review-actions.md](docs/crypto-review-actions.md)。
