# ML-KEM 文件加密示例

使用 ML-KEM-1024、HKDF-SHA256、AES-256-GCM 加密文件，并用 ML-DSA-87 验证发送方身份。默认写入带签名的文件格式 v2；旧 v1 文件只能显式选择兼容模式读取。这是自定义格式的学习示例，不是独立审计或经过 FIPS 认证的产品。

## 使用

```sh
# 接收方生成加密密钥，默认必须设置非空密码，并确认一次
cargo run --release --bin decrypt -- --keygen recipient-v2.public recipient-v2.private

# 发送方独立生成签名密钥，设置其自己的非空密码
cargo run --release --bin decrypt -- --sign-keygen sender.public sender.private

# 加密：输入发送方签名私钥密码（旧受保护公钥会先要求其密码）
cargo run --release --bin encrypt -- --sign-key sender.private recipient-v2.public input.txt encrypted-v2.bin

# 解密：先验证可信发送方的签名，再输入接收方私钥密码
cargo run --release --bin decrypt -- --verify-key sender.public recipient-v2.private encrypted-v2.bin recovered-v2.txt
```

`sender.public` 必须预先经可信渠道交付并核对指纹，不能直接信任与密文一起从可被替换的 URL 下载的公钥。签名私钥由发送方独立保存，接收方私钥由接收方独立保存；两者不能混用，签名私钥及其密码不应随接收方公钥分发。

新生成的接收方公钥默认裸存，仅私钥需要密码；公钥无需保密，加密时因此少一次 Argon2。需要维持公钥密码保护时，使用 `--keygen --protect-public <公钥> <私钥>`，此时公私钥仍使用同一密码，不能同时指定 `--no-password`。旧 `--public-unprotected` 选项作为新默认行为的兼容别名保留。签名公钥始终裸存，以便先验证签名。

已有加密公钥不会自动解密或改写，读取它仍会提示输入原密码。新默认值也不会改变已有私钥的 KDF 参数。若将受保护公钥密码交给其他发送方，同一密码也会解锁泄漏的接收方私钥，因此公钥密码保护不构成身份隔离。

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

新密钥使用 Argon2id v0x13，64 MiB / t=3，p 取 `available_parallelism()` 的 1–4 范围（查询失败回退 1），并写入密钥头部；开启 `argon2/parallel`，通过 Rayon 并行处理各 lane。64 MiB 是总内存，不会乘以 p。旧密钥严格使用头部记录的参数，包括 lanes=1。读入仅接受内存 64–256 MiB、t=3–10、p=1–4，参数在分配内存前检查。不宣称是 RFC 9106 第一推荐档；RFC 的 64 MiB 第二档示例使用 p=4，详情见 [RFC 9106 §4](https://www.rfc-editor.org/rfc/rfc9106.html#section-4)。

## 库接口和文件保护

- `format`：不分配内存的格式/长度校验；`kdf`：密码派生与版本化 HKDF。
- `keys`：密钥容器和指纹；`signing`：签名及显式信任策略；`crypto`：加解密流程。
- `file`：有界读取、排他写入及同步；`platform`：Windows ACL；`src/cli`：终端密码输入。
- `PasswordSource` 由调用方提供，支持 `FixedPassword`、`NoPassword` 或自定义实现。库没有 stdin/TTY/inquire 调用。
- `Error` 为结构化枚举，可匹配 `KeyUnlockFailed`、`SignatureInvalid`、`RecipientMismatch`、`AuthenticationFailed`、`UnsupportedVersion`、`KdfParameters` 等；密码错误与密钥被篡改统一报 `KeyUnlockFailed`，不承诺区分这两种情况。

文件读取上限为 64 MiB 明文，拒绝非普通文件以及读取时长度变化；同长度并发改写不能仅通过长度检测发现。密码、私钥、派生字节、明文和 Argon2 工作内存用 `Zeroizing` 或 `Zeroize` 擦除；不覆盖 inquire 内部缓冲、交换区、崩溃转储和调用方自己保留的副本，也不提供 mlock。

Unix 新文件权限为 0600，并同步文件及父目录；Windows 使用仅 Owner Rights 可访问且禁止继承的 DACL，需要支持 ACL 的文件系统。签名验证后，按 FIPS 203 的展开私钥结构提取内嵌公钥，检查其 SHA3-256 哈希，再用 SHA-256 指纹比对头部接收方；不匹配返回 `RecipientMismatch`。此过程不依赖后端对导入私钥不支持的 `encapsulation_key()`。签名、接收方和 AEAD 均验证成功后才创建明文输出；格式与 64 MiB 上限不变，未引入流式 AEAD。

KDF 工作区只在一次操作内复用：同一批公钥/签名私钥解锁或公私钥保护共用一块缓冲；加密路径在加载明文前擦除并释放这块缓冲。单独 `read_key`/`unlock_key` 仍在返回时擦除。缓冲扩容前先擦除旧数据，正常退出、错误返回和 panic 展开均走 Drop；不使用不保证析构的进程全局缓存。复用会把第一次 KDF 中间状态的最长保留时间延长到批次结束（可能包含第二次密码输入等待）。密钥头部每次读取只解析一次。

## 测试与性能

```sh
cargo fmt -p encrypt_file -- --check
cargo clippy -p encrypt_file --all-targets --all-features -- -D warnings
cargo test -p encrypt_file --release --lib --tests
cargo bench
cargo bench --features perf-trace
cargo bench --bench cli -- protected/encrypt/32MiB --samples 5
cargo bench --bench cli -- private-only/encrypt/32MiB --samples 5
```

基准为原生 Rust，27 个场景：plain（无密码）、private-only（新默认：公钥裸存、两种私钥受保护）、protected（接收方公私钥均受保护）；每种包含 keygen 和 4 B / 1 MiB / 32 MiB / 64 MiB 加解密。各预热一次，默认采样三次，输出中位数/min/max。签名密钥生成作为不计时的准备阶段。计时包含进程启动、磁盘同步和擦除，排除输入等待、准备、校验、清理；临时数据不影响用户文件，不采集 CPU 时间/峰值 RSS。

`perf-trace` 父子阶段有包含关系，不能直接相加。工作区擦除现在发生在 KDF 批次结束，`kdf.total` 不再包含这次集中擦除；完整耗时应看 `command.*` 或 CLI 总耗时。对比优化应注明文件大小、保护策略和 KDF 参数；不能把减少一次 KDF 当作同参数并行加速。

`tests/fixtures` 含固定旧 v1 文件和 NIST 官方 ACVP ML-KEM-1024 已知答案，包含隐式拒绝测试，来源与哈希见其中 README。它们的私钥是公开测试材料，不能用于实际数据。此类回归测试不等于算法或产品认证。

fuzz 目标和运行方法见 [fuzz/README.md](fuzz/README.md)。仓库根 `.github/workflows/encrypt-file.yml` 配置 macOS/Windows 的 fmt、Clippy、回归测试；只编译 benchmark，不在 CI 执行基准。Windows 测试包含 DACL 检查，本地 macOS 成功不能代替 Windows runner 的结果。

本地评审笔记、编辑器备份与 CLI 生成文件由 `.gitignore` 排除；测试用公开密钥和已知答案仍纳入版本控制。

### 本次优化实测（32 MiB，release）

本机 i9-9980HK，每场景预热一次、采样 3 次取中位数，包含进程启动、签名、文件同步及擦除，不含编译和人工输入。旧基线为串行 lanes=1；新密钥为 parallel + lanes=4。m=64 MiB、t=3 不变，下面的变快不代表旧密钥会自动变成 lanes=4。

| 模式 | CLI 总耗时 | Argon2 合计 | KDF 分配 | KDF 擦除 |
|---|---:|---:|---:|---:|
| 优化前，双密码保护 | 658.2 ms | 297.5 ms（2 次） | 47.2 ms（2 次） | 80.7 ms（2 次） |
| 优化后，双密码保护 | 369.9 ms | 85.1 ms（2 次） | 37.2 ms（1 次） | 7.5 ms（1 次） |
| 优化后，新默认裸公钥 | 328.3 ms | 43.1 ms（1 次） | 38.3 ms（1 次） | 7.5 ms（1 次） |

新工作区按 Block 擦除所有曾使用的区域，再释放 Vec，不重复擦除同一缓冲；没有跳过敏感内存清除。旧密钥按照保存的 lanes 读取，若需要新的并行参数，应使用新文件名新建密钥，不自动覆盖旧密钥。
