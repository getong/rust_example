pub mod advanced;
pub mod basic;

pub use advanced::AdvancedRenderer;
pub use basic::BasicRenderer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
  Basic,
  Advanced,
}

impl RenderMode {
  pub fn toggle(self) -> Self {
    match self {
      Self::Basic => Self::Advanced,
      Self::Advanced => Self::Basic,
    }
  }
}
