# ML-KEM 文件加密 demo

使用 aws-lc-rs：ML-KEM-1024 封装共享秘密，经 HKDF-SHA256 派生 AES-256-GCM 密钥。
每次加密重新封装，并生成随机盐和 nonce。保留 `src/encrypt.rs`、`src/decrypt.rs`、`src/common.rs` 三个源文件。

```sh
# 接收方生成密钥对，只需要生成一次
cargo run --bin decrypt -- --keygen recipient.public recipient.private

# 加密端只需要接收方公钥；input.txt 是自己准备的文件
cargo run --bin encrypt -- recipient.public input.txt encrypted.bin

# 接收方使用私钥解密
cargo run --bin decrypt -- recipient.private encrypted.bin recovered.txt

cargo test -p encrypt_file
```

公私钥采用 aws-lc-rs ML-KEM-1024 原始二进制格式，不是 PEM：公钥 1568 字节，私钥 3168 字节。
私钥文件没有口令加密，必须独立保管、备份，不能随密文公开；Unix 创建权限为 0600，其他平台需配置访问权限。
公钥可以公开，但加密端必须通过可信渠道确认公钥归属。此方案不提供发送方身份认证或数字签名。

所有输出均使用排他创建，拒绝覆盖现有文件。只有完整认证通过后才写入解密文件。
密钥生成先写私钥再写公钥；若第二次写入失败，已写入的私钥会保留，需要检查报错并使用新的路径重新生成配对密钥。
发生磁盘写入错误时，可能留下不完整输出；程序不会自动删除或覆盖这些文件。

## v1 文件格式

| 字段 | 字节数 |
| --- | ---: |
| 魔数 `ALCFENC\0` | 8 |
| 版本 `1` | 1 |
| 算法套件 `1`（ML-KEM-1024 / HKDF-SHA256 / AES-256-GCM） | 1 |
| HKDF 随机盐 | 32 |
| ML-KEM 封装密文 | 1568 |
| AES-GCM nonce | 12 |
| 文件密文 | 与明文等长 |
| GCM 认证标签 | 16 |

前 1622 字节全部作为 GCM AAD 认证。HKDF info 固定为
`encrypt_file/v1/ML-KEM-1024/HKDF-SHA256/AES-256-GCM`，输出 32 字节密钥。
文件不保存私钥、共享秘密或 AES 密钥。旧的口令加密 demo 文件不兼容此格式。

这是自定义文件格式的学习示例，当前整文件读入内存，适用于小文件，不是经过审计的生产文件加密协议。
