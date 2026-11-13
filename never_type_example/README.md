# Rust never type（`!`）示例

## 版本说明

截至 2026-11-13，never type 已在 **2026-11-12 发布的 Rust 1.100.0 stable** 中稳定化，不再只是 nightly 功能。

- `fn stop() -> !` 这种返回类型的写法早已可在稳定版使用。
- Rust 1.100 新增的是在一般类型位置直接使用 `!`，例如 `Result<T, !>`、`Option<!>` 和 `type Err = !`。
- 本项目最低要求 Rust 1.100，无第三方依赖，不需要 `#![feature(never_type)]`。
- Rust 1.100 同时将 `std::convert::Infallible` 改为 `!` 的类型别名；兼容旧编译器的代码仍可使用 `Infallible`。

## 运行

```bash
cargo +stable run
cargo +nightly run
```

编译器必须至少为 1.100。默认运行会依次演示六组用法，正常结束，不会主动 panic、退出进程或启动无限循环。

单独体验不会正常返回的函数：

```bash
cargo +stable run -- --panic
```

该命令会故意 panic 并以非零状态退出；这是演示行为，不是程序错误。

## `!` 究竟表示什么？

`!` 是 **没有任何合法值的类型**（uninhabited type）。它表达的是“这条执行路径不可能产生一个值”，而不是“返回一个空值”。

| 类型或写法 | 含义 | 能否正常返回 |
| --- | --- | --- |
| `()` | unit 类型，有且只有一个值 `()` | 可以 |
| `fn work()` | 等价于 `fn work() -> ()` | 可以 |
| `fn stop() -> !` | 不会正常返回到调用处 | 不可以 |
| `Option<!>` | `Some` 的内容不可能构造，因此只有 `None` | 可以返回 `None` |
| `Result<T, !>` | `Err` 的内容不可能构造 | 可以返回 `Ok(T)` |
| `Result<!, E>` | `Ok` 的内容不可能构造 | 可以返回 `Err(E)` |

“不正常返回”不只包括无限循环，也包括 panic、终止进程等。panic 在 unwind 模式下可以在外层被捕获，但这不代表产生 panic 的 `-> !` 函数正常返回了。

## 示例对应的实际作用

### 1. 用函数签名描述控制流

`fail_fast` 的返回类型是 `!`：

```rust
fn fail_fast(message: &str) -> ! {
    panic!("{message}")
}
```

调用者和编译器都知道：这个调用之后的正常路径不可达。适用于不可恢复错误处理、进程退出入口、永不结束的事件循环。

不要给会正常结束的函数标注 `-> !`，也不要把原本可以恢复的错误一律改成 panic。

### 2. 让提前退出的分支参与类型检查

`require_value` 中，一个分支产生 `u32`，另一个调用 `-> !` 函数；后者可以在此处强制转换为 `u32`，因此整个 `match` 类型正确。

`double_if_present` 同时演示了 `return None`：**函数返回的是 `Option<u32>`，但 `return None` 这个表达式本身的类型是 `!`**，因为执行不会继续到当前表达式之后。

这种 never-to-any coercion 不是凭空制造任意类型的值；它之所以安全，正是因为该路径不会产生值。

### 3. 解释 `continue` 和 `break` 为什么能放进表达式

`sum_until_stop` 在需要 `u32` 的 `match` 中使用 `break` 和 `continue`：

- `"invalid"` 解析失败，`continue` 跳到下一轮。
- `"stop"` 触发 `break`，退出循环。
- `"100"` 位于停止标记之后，不参与计算。
- 最终结果为 `30`。

注意区分：`break value` 表达式本身的类型是 `!`，但外层 `loop` 表达式可以因为它而产生 `value` 对应类型的值。只有不会通过 `break` 结束的无限 `loop` 才是 `!`。

### 4. 在泛型接口中表达“不可能出错”

`default_port` 返回 `Result<u16, !>`；`infallible_unwrap` 可以直接拆出 `Ok`：

```rust
fn infallible_unwrap<Value>(result: Result<Value, !>) -> Value {
    let Ok(value) = result;
    value
}
```

这里没有 `unwrap()`，也没有隐藏的错误分支。编译器知道 `Err(!)` 无法构造，因此不需要处理它。

普通函数如果没有统一接口约束，直接返回 `u16` 往往更简单；`Result<T, !>` 主要用于必须返回 `Result` 的 trait 或泛型接口。“不可能出错”指不可能返回 `Err`，并不承诺永不 panic 或一定终止。

### 5. 为 trait 选择永远不发生的错误类型

`RawText` 接受任意字符串，实现 `FromStr` 时设置 `type Err = !`。

这样既遵守 `FromStr` 必须返回 `Result` 的接口，又在类型层面承诺不会返回解析错误。调用方直接使用 `let Ok(text) = ...` 即可。

### 6. 禁用枚举中不可能的状态

`simulate_service_failure` 返回 `Result<!, &str>`：它不能返回 `Ok`，但可以返回 `Err`。这样的接口适合“成功启动后持续运行，只在失败时返回”的服务。

示例只模拟失败，不启动真实服务；调用方只需 `let Err(error) = ...`。

`Option<!>` 则说明：包含 `!` 的类型不一定完全没有值。`Some(!)` 不可能存在，但 `None` 仍然合法，匹配时只写 `None` 就足够了。

## 一句话总结

`!` 把“不可能产生值 / 不可能正常返回 / 不可能出现某个分支”写进类型系统，让控制流和泛型接口表达得更准确，并让编译器检查这些约束。

## 官方资料

- Rust 1.100.0 发布说明：`https://blog.rust-lang.org/2026/11/12/Rust-1.100.0/`
- never 类型文档：`https://doc.rust-lang.org/std/primitive.never.html`
- Infallible 文档：`https://doc.rust-lang.org/std/convert/type.Infallible.html`
