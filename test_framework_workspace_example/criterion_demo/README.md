# criterion_demo

`criterion` 是 Rust 常用 benchmark 框架。相比简单计时，它会做统计采样，能输出更稳定的性能结果，并可通过 `html_reports` 生成 HTML 报告。

本项目展示：

- 把可测逻辑放在 `src/lib.rs`。
- 在 `benches/` 中编写 Criterion benchmark。
- 使用 `black_box` 降低编译器把 benchmark 优化掉的风险。

运行：

```sh
cargo bench -p criterion_demo
```

HTML 报告通常会生成在 `target/criterion/report/index.html`。
