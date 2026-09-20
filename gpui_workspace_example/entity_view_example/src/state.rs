//! 原生 GPUI：领域状态用 Entity，全局配置用 Global。
use gpui_kit::{Context, Global};

#[derive(Clone, Copy, Debug)]
pub enum CounterId {
  A,
  B,
}

impl CounterId {
  pub fn name(self) -> &'static str {
    match self {
      Self::A => "A",
      Self::B => "B",
    }
  }
  fn index(self) -> usize {
    match self {
      Self::A => 0,
      Self::B => 1,
    }
  }
}

// Global 按类型存储在 App 中，是应用级配置，而非进程级静态变量。
// 创建窗口前 set_global；读取 global；修改 update_global 会通知观察者。
pub struct AppSettings {
  step: usize,
}
impl Global for AppSettings {}
impl Default for AppSettings {
  fn default() -> Self {
    Self { step: 1 }
  }
}
impl AppSettings {
  pub fn step(&self) -> usize {
    self.step
  }
  pub fn toggle_step(&mut self) {
    self.step = if self.step == 1 { 5 } else { 1 };
  }
}

// Model 不需要实现 Render 或某个 Model trait，只需由 cx.new 托管为 Entity。
// 多个视图共享这个实体，避免复制状态及视图间互相强引用。
pub struct CounterState {
  counts: [usize; 2],
  last_change: String,
}
impl Default for CounterState {
  fn default() -> Self {
    Self {
      counts: [0, 0],
      last_change: "Waiting for a counter update".into(),
    }
  }
}
impl CounterState {
  pub fn count(&self, id: CounterId) -> usize {
    self.counts[id.index()]
  }
  pub fn total(&self) -> u128 {
    self.counts.iter().map(|&value| value as u128).sum()
  }
  pub fn last_change(&self) -> &str {
    &self.last_change
  }

  pub fn increment(&mut self, id: CounterId, cx: &mut Context<Self>) {
    // Entity 更新可以读取应用 Global。本次操作使用当前配置。
    let step = cx.global::<AppSettings>().step();
    self.counts[id.index()] = self.count(id).saturating_add(step);
    self.last_change = format!("Counter {} changed to {}", id.name(), self.count(id));
    cx.notify(); // 通知这个 Model 的观察者，观察者决定刷新哪个视图。
  }

  pub fn reset(&mut self, cx: &mut Context<Self>) {
    // 同一次 update 中修改两个值，观察者读取到完整的重置结果。
    self.counts = [0, 0];
    self.last_change = "Both counters reset".into();
    cx.notify();
  }
}
