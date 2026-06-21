# clatter_example

This example uses `clatter` to build a fixed-buffer Noise handshake with
post-quantum key material:

```text
Noise_hybridNN_X25519+MLKEM512_AESGCM_SHA512
```

`clatter` is the protocol state machine. It applies the Noise handshake pattern,
mixes the handshake hash/KDF, runs the X25519 DH exchange and ML-KEM KEM
operation, derives transport keys, then exposes `send`/`receive` for
authenticated encryption.

效果:

- 握手阶段同时混入经典 X25519 和后量子 ML-KEM 的共享密钥材料。
- 握手完成后双方得到匹配的传输密钥。
- 传输阶段消息会被 AES-GCM 加密和认证，接收方可以验证并解密。
- `src/lib.rs` 不使用 `std` API，也不使用堆分配；固定缓冲区适合移植到
  `no_std` 目标。

Run the host demo:

```sh
cargo run
```

Check only the `no_std` library path without the host RNG feature:

```sh
cargo check --lib --no-default-features
```

For bare-metal targets, call `run_hybrid_exchange_with_rng::<YourRng>()`, where
`YourRng` implements `clatter::traits::Rng`.
