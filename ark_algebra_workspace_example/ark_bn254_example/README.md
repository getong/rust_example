# ark_bn254 ZKP Primitives Showcase

展示 BN254 (alt_bn128) 椭圆曲线上的零知识证明基础原语，基于 `ark-bn254` 库。

## Rust + ZKP 功能与作用

### BN254 曲线参数

| 参数 | 值 |
|------|-----|
| 标量域 Fr | 254 bits |
| 基域 Fq | 254 bits |
| 嵌入度 | 12 |
| 配对类型 | Ate pairing, GT ⊂ Fq¹² |

BN254 是 **pairing-friendly 椭圆曲线**，支持高效的双线性配对运算 `e: G₁ × G₂ → GT`。它是以太坊 EVM 预编译（0x06, 0x07, 0x08）使用的曲线，也是 Groth16、Plonk 等主流 zkSNARK 系统的常用选择。

### 代码涵盖的原语

| 模块 | 对应 ZKP 概念 | 作用 |
|------|---------------|------|
| **Arithmetic circuit** | 算术约束系统 | 将计算表示为域上的多项式约束，witness 满足约束即证明计算正确 |
| **Pedersen commitment** | 承诺方案 | 绑定 value + 随机数到一个群元素，隐藏值的同时承诺不可篡改 |
| **Aggregated commitment** | 批量承诺 / MSM | 用多标量乘法同时承诺多个值，Confidential Transactions 的核心 |
| **Sigma protocol** | 知识证明（非交互） | 通过 Fiat-Shamir 变换将交互式证明转为非交互式，证明知道离散对数 |
| **BLS signature** | 配对签名 | 利用 `e(σ, G₂) = e(H(m), pk)` 实现短签名和聚合签名 |
| **Inner product** | 内积论证 | `⟨a, b⟩ = c` 是 Bulletproofs 和 Range Proofs 的核心关系 |
| **GT target group** | 目标群运算 | 配对输出在 GT 中，Groth16 验证器需要 GT 上的乘法和幂运算 |
| **Field operations** | 域运算 | 逆元、平方根、Legendre 符号等，ZK 电路内部的基本运算单元 |

### ZKP 应用场景

- **隐私支付**：证明余额充足且交易合法，不泄漏金额和交易方（Zcash, Tornado Cash）
- **身份凭证**：证明年龄 ≥ 18 而不泄露生日（zkCreds, AnonCreds）
- **L2 Rollup**：将数千笔交易聚合成一个 zkProof，L1 只需验证一个配对（zkSync, Scroll）
- **可验证计算**：外包计算并提供证明，验证比重新计算快得多（zkVM, Proveable）

## 运行

```bash
cargo run
cargo test
```
