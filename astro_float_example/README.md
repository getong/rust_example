# astro_float example

这个目录演示 `astro-float` 的核心能力：用纯 Rust 的 `BigFloat` 做任意精度浮点运算，并显式控制精度、舍入模式和进制格式化。

## 它解决什么问题

`f32`/`f64` 的精度和指数范围是固定的，适合大多数工程计算，但在高精度科学计算、数值验证、常量计算、误差敏感的中间步骤中可能不够用。`astro-float` 允许你为每次计算指定 mantissa 的 bit 精度，并提供正确舍入的基础运算和常见数学函数。

这个示例展示了：

- `BigFloat` 任意精度浮点数。
- `Consts` 常量缓存：`pi`、`e`、`ln(2)`、`ln(10)` 等会按需计算并缓存。
- `Context` 和 `expr!`：用接近数学公式的语法写 `sqrt`、`ln`、`exp`、`pow`、`sin`、`cos` 等表达式。
- `RoundingMode`：控制结果舍入方式，例如 `ToEven`、`Up`、`Down`、`ToZero`。
- `Radix`：支持二进制、八进制、十进制、十六进制解析和格式化。
- 浮点特殊值：`NaN`、`Inf`、`-Inf`。

## 运行

```sh
cargo run
```

## 注意

`astro-float` 是任意精度浮点，不是十进制定点账本类型。它适合需要高精度浮点和数学函数的场景；如果业务要求十进制金额的严格账务语义，通常应使用 decimal/fixed-point 类型。
