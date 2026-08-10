#pragma once

#include <cstddef>
#include <cstdint>

#include "rust/cxx.h"

// tokio 调用 asio 的 FFI 表面（与 asio-call-tokio 项目正好镜像）。
//
// 协程本身无法跨语言边界，所以桥的形状是 completion callback：
// 下面每个函数都在把一个 C++ 协程扔到后台线程的 asio io_context 上之后
// 立即返回；协程算完后从 asio 线程回调 Rust 侧的 complete_*（见
// src/main.rs 里 #[cxx::bridge] 的 extern "Rust" 部分），把结果连同
// 不透明的 ctx 一起送回去。ctx 里装的是 tokio oneshot channel 的发送端，
// 因此 Rust 侧一个普通的 .await 就在等 C++ asio 协程。
namespace asio_ffi {

/// 在 asio 的 steady_timer 上等 delay_ms，然后把 lhs + rhs
/// 经 complete_add(ctx, sum) 从 asio 线程送回。
void sleep_then_add(std::int32_t lhs, std::int32_t rhs, std::uint64_t delay_ms,
                    std::size_t ctx);

/// 模拟一次异步获取：等 delay_ms 后把为 name 生成的问候语
/// 经 complete_greet(ctx, text) 送回。
void fetch_greeting(rust::Str name, std::uint64_t delay_ms, std::size_t ctx);

/// 撤掉 io_context 的 work guard 并 join 后台线程；
/// 在 Rust main 退出前调用，保证干净关闭。
void shutdown();

} // namespace asio_ffi
