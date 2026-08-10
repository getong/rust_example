#include "src/api.h"

// cxx 生成的桥头文件，声明了 extern "Rust" 的 complete_add / complete_greet
#include "tokio_call_asio/src/main.rs.h"

#include <boost/asio/co_spawn.hpp>
#include <boost/asio/detached.hpp>
#include <boost/asio/executor_work_guard.hpp>
#include <boost/asio/io_context.hpp>
#include <boost/asio/steady_timer.hpp>
#include <boost/asio/this_coro.hpp>
#include <boost/asio/use_awaitable.hpp>

#include <chrono>
#include <sstream>
#include <string>
#include <thread>

namespace asio = boost::asio;

namespace
{

/// 进程级的 asio 引擎：一个 io_context + 跑 run() 的后台线程。
/// work guard 让 run() 在没有待处理任务时也不退出，
/// 直到 shutdown() 撤掉它——正好镜像参考项目里 tokio 的全局 Runtime。
struct Engine
{
  asio::io_context ioc;
  asio::executor_work_guard<asio::io_context::executor_type> work {
      ioc.get_executor()};
  std::thread runner {[this] { ioc.run(); }};

  ~Engine() { stop(); }

  void stop()
  {
    work.reset();
    if (runner.joinable()) {
      runner.join();
    }
  }
};

Engine& engine()
{
  static Engine e;
  return e;
}

std::string thread_id_string()
{
  std::ostringstream oss;
  oss << std::this_thread::get_id();
  return oss.str();
}

asio::awaitable<void> sleep_for(std::uint64_t delay_ms)
{
  auto timer = asio::steady_timer {co_await asio::this_coro::executor};
  timer.expires_after(std::chrono::milliseconds {delay_ms});
  co_await timer.async_wait(asio::use_awaitable);
}

}  // namespace

namespace asio_ffi
{

void sleep_then_add(std::int32_t lhs,
                    std::int32_t rhs,
                    std::uint64_t delay_ms,
                    std::size_t ctx)
{
  asio::co_spawn(
      engine().ioc,
      [lhs, rhs, delay_ms, ctx]() -> asio::awaitable<void>
      {
        co_await sleep_for(delay_ms);
        // 从 asio 线程回调 Rust；ctx 会被 Rust 侧的 complete_add
        // 恰好消费一次（里面是 oneshot::Sender）。
        complete_add(ctx, lhs + rhs);
      },
      asio::detached);
}

void fetch_greeting(rust::Str name, std::uint64_t delay_ms, std::size_t ctx)
{
  asio::co_spawn(
      engine().ioc,
      // rust::Str 只是借用，协程会晚于本函数返回才执行，必须先拷成 std::string
      [name = std::string(name), delay_ms, ctx]() -> asio::awaitable<void>
      {
        co_await sleep_for(delay_ms);
        auto text = "Hello, " + name
            + "! Composed by an asio coroutine on thread "
            + thread_id_string() + ".";
        complete_greet(ctx, rust::String(text));
      },
      asio::detached);
}

void shutdown()
{
  engine().stop();
}

}  // namespace asio_ffi
