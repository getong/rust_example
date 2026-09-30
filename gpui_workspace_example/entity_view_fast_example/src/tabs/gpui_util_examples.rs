//! gpui_util Tab 的案例逻辑，结果由交互页面展示。
use std::{any::TypeId, cell::Cell, collections::HashMap, sync::Arc};

use gpui_util::{ResultExt as _, TryFutureExt as _, arc_cow::ArcCow};

pub fn sharing() -> String {
  let borrowed: ArcCow<'_, str> = "静态标题".into();
  let owned: ArcCow<'_, str> = String::from("动态标题").into();
  let cloned = owned.clone();
  let shared = match (&owned, &cloned) {
    (ArcCow::Owned(a), ArcCow::Owned(b)) => Arc::ptr_eq(a, b),
    _ => false,
  };
  format!(
    "借用：{}；拥有：{}；克隆共享同一 Arc：{shared}",
    borrowed.as_ref(),
    owned.as_ref()
  )
}

pub fn errors() -> String {
  let valid = "8080".parse::<u16>().log_err();
  let invalid = "abc".parse::<u16>().warn_on_err();
  let local: Option<u16> = gpui_util::maybe!({
    let port = valid?;
    (port > 0).then_some(port)
  });
  // ready 保证立即完成：此处仅演示 Future 适配，不在 UI 线程等待 I/O。
  let future =
    futures::executor::block_on(futures::future::ready(Err::<u16, _>("模拟异步失败")).log_err());
  format!("有效输入：{valid:?}；无效输入：{invalid:?}；maybe!：{local:?}；异步错误：{future:?}")
}

pub fn cleanup(cancel: bool) -> String {
  let cleaned = Cell::new(false);
  {
    // 必须绑定 guard；直接丢弃会立即执行清理。
    let guard = gpui_util::defer(|| cleaned.set(true));
    if cancel {
      guard.abort();
    }
  }
  format!("取消清理：{cancel}；离开作用域后已清理：{}", cleaned.get())
}

pub fn measurement() -> String {
  let total = gpui_util::measure("gpui-util-demo sum", || (1_u64 ..= 10_000).sum::<u64>());
  format!("1..=10000 求和 = {total}；启动前设置 ZED_MEASUREMENTS=1 可在 stderr 查看耗时。")
}

pub fn helpers() -> String {
  let mut next_id = 41_u32;
  let assigned = gpui_util::post_inc(&mut next_id);
  let mut registry = HashMap::with_hasher(gpui_util::TypeIdHashBuilder);
  registry.insert(TypeId::of::<String>(), "文本服务");
  // 只构造命令，不启动外部进程。
  let mut command = gpui_util::new_std_command("rustc");
  command.arg("--version");
  format!(
    "分配 ID：{assigned}；下个 ID：{next_id}；类型注册表：{:?}；命令（未执行）：{command:?}",
    registry.get(&TypeId::of::<String>())
  )
}
