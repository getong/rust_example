use std::time::{Duration, Instant};

const HIDE_DELAY: Duration = Duration::from_secs(5);

pub struct AutoHide {
  last_activity: Instant,
  hidden: bool,
}

impl AutoHide {
  pub fn new(now: Instant) -> Self {
    Self {
      last_activity: now,
      hidden: false,
    }
  }

  pub fn show(&mut self, now: Instant) -> bool {
    self.last_activity = now;
    std::mem::take(&mut self.hidden)
  }

  pub fn update(&mut self, now: Instant, can_hide: bool) -> bool {
    if !can_hide {
      return self.show(now);
    }
    let hidden = now.saturating_duration_since(self.last_activity) >= HIDE_DELAY;
    let changed = self.hidden != hidden;
    self.hidden = hidden;
    changed
  }

  pub fn hidden(&self) -> bool {
    self.hidden
  }
}
