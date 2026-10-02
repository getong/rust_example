# 解密路径 fuzz

需要 nightly Rust 和 cargo-fuzz：

```sh
cargo install cargo-fuzz --locked
cargo run --manifest-path fuzz/Cargo.toml --example seed_corpus
cargo +nightly fuzz run decrypt -- -max_len=131072 -max_total_time=60
```

上述命令从本包目录运行。目标覆盖文件/密钥格式解析、签名校验、旧格式 KEM/AEAD 解密；不会提示密码、创建文件或执行恶意输入指定的 Argon2 参数。单个输入限制 128 KiB；这不是 64 MiB 上限的资源压力测试。

Rust 种子生成器将固定 v1/v2、有效 v3 和密钥头部样本写入 `fuzz/corpus/decrypt/`，以覆盖认证通过后的路径。测试密钥为公开 fixture，不能用于实际数据。

在不安装 cargo-fuzz 时也可编译：`cargo check --manifest-path fuzz/Cargo.toml`。仅编译或短时 smoke run 不构成充分 fuzz 审计；长期运行发现的输入保存到 corpus。
