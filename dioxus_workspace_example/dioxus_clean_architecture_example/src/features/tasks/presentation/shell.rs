use dioxus::prelude::*;

use super::{
  bloc::use_task_bloc,
  pages::{Overview, Settings, Tasks},
};

#[derive(Clone, Debug, PartialEq, Routable)]
#[rustfmt::skip]
pub enum TaskRoute {
  #[layout(AppShell)]
    #[route("/")]
    Tasks {},
    #[route("/overview")]
    Overview {},
    #[route("/settings")]
    Settings {},
}

#[component]
fn AppShell() -> Element {
  let route: TaskRoute = use_route();
  let storage_available = use_task_bloc().is_available();

  rsx! {
    div { class: "app-shell",
      header { class: "top-bar",
        div { class: "brand-mark", "F" }
        div {
          p { class: "brand-name", "Focus Board" }
          p { class: "brand-date", "本地任务空间" }
        }
        span {
          class: if storage_available { "storage-status" } else { "storage-status error" },
          if storage_available { "已连接" } else { "异常" }
        }
      }

      main { class: "page-content",
        Outlet::<TaskRoute> {}
      }

      nav { class: "bottom-nav", aria_label: "主导航",
        NavItem {
          to: TaskRoute::Tasks {},
          label: "任务",
          symbol: "✓",
          active: matches!(route, TaskRoute::Tasks {}),
        }
        NavItem {
          to: TaskRoute::Overview {},
          label: "概览",
          symbol: "▦",
          active: matches!(route, TaskRoute::Overview {}),
        }
        NavItem {
          to: TaskRoute::Settings {},
          label: "设置",
          symbol: "⚙",
          active: matches!(route, TaskRoute::Settings {}),
        }
      }
    }
  }
}

#[component]
fn NavItem(to: TaskRoute, label: &'static str, symbol: &'static str, active: bool) -> Element {
  rsx! {
    Link {
      to,
      class: if active { "nav-item active" } else { "nav-item" },
      aria_label: label,
      span { class: "nav-symbol", aria_hidden: "true", {symbol} }
      span { class: "nav-label", {label} }
    }
  }
}
