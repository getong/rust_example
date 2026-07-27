use dioxus::prelude::*;

use super::bloc::{TaskEvent, TaskFilter, use_task_bloc};
use crate::features::tasks::domain::Task;

#[component]
pub fn Tasks() -> Element {
  let bloc = use_task_bloc();
  let state = bloc.snapshot();
  let mut draft = use_signal(String::new);
  let can_add = !draft.read().trim().is_empty();
  let visible_tasks = state.visible_tasks();

  rsx! {
    section { class: "page",
      div { class: "page-heading",
        div {
          p { class: "eyebrow", "今日" }
          h1 { "任务" }
        }
        span { class: "task-count", "{state.active_count()} 项待完成" }
      }

      if let Some(error) = state.error.as_deref() {
        ErrorBanner { message: error.to_owned() }
      }

      form {
        class: "add-task",
        onsubmit: {
          let mut bloc = bloc.clone();
          move |event| {
            event.prevent_default();
            let title = draft.read().clone();
            if !title.trim().is_empty() {
              bloc.dispatch(TaskEvent::Add(title));
              draft.set(String::new());
            }
          }
        },
        input {
          value: "{draft}",
          oninput: move |event| draft.set(event.value()),
          placeholder: "添加一项任务",
          aria_label: "任务标题",
          maxlength: 120,
        }
        button {
          r#type: "submit",
          class: "add-button",
          disabled: !can_add,
          aria_label: "添加任务",
          title: "添加任务",
          span { aria_hidden: "true", "+" }
        }
      }

      div { class: "filter-bar", role: "group", aria_label: "任务筛选",
        FilterButton { label: "全部", filter: TaskFilter::All, selected: state.filter == TaskFilter::All }
        FilterButton { label: "进行中", filter: TaskFilter::Active, selected: state.filter == TaskFilter::Active }
        FilterButton { label: "已完成", filter: TaskFilter::Completed, selected: state.filter == TaskFilter::Completed }
      }

      div { class: "task-list",
        if visible_tasks.is_empty() {
          div { class: "empty-state",
            div { class: "empty-symbol", aria_hidden: "true", "✓" }
            h2 { "这里很清爽" }
            p { "当前筛选下没有任务" }
          }
        } else {
          for task in visible_tasks {
            TaskRow { key: "{task.id}", task }
          }
        }
      }
    }
  }
}

#[component]
pub fn Overview() -> Element {
  let state = use_task_bloc().snapshot();
  let completed = state.completed_count();
  let active = state.active_count();
  let percent = state.completion_percent();

  rsx! {
    section { class: "page",
      div { class: "page-heading",
        div {
          p { class: "eyebrow", "进度" }
          h1 { "概览" }
        }
      }

      div { class: "progress-panel",
        div { class: "progress-copy",
          span { class: "progress-value", "{percent}%" }
          span { class: "progress-label", "总体完成率" }
        }
        div { class: "progress-track",
          div { class: "progress-fill", style: "width: {percent}%" }
        }
      }

      div { class: "metric-grid",
        Metric { value: state.tasks.len(), label: "任务总数", tone: "ink" }
        Metric { value: active, label: "正在进行", tone: "coral" }
        Metric { value: completed, label: "已经完成", tone: "green" }
      }

      div { class: "section-heading",
        h2 { "最近任务" }
        span { "{state.tasks.len().min(4)} 条" }
      }
      div { class: "compact-list",
        if state.tasks.is_empty() {
          p { class: "quiet-empty", "暂无任务记录" }
        } else {
          for task in state.tasks.iter().take(4) {
            div { class: "compact-row",
              span { class: if task.completed { "status-dot done" } else { "status-dot" } }
              span { class: if task.completed { "compact-title done" } else { "compact-title" }, "{task.title}" }
              span { class: "compact-status", if task.completed { "完成" } else { "进行中" } }
            }
          }
        }
      }
    }
  }
}

#[component]
pub fn Settings() -> Element {
  let bloc = use_task_bloc();
  let state = bloc.snapshot();
  let database_path = bloc.database_path().display().to_string();
  let storage_available = bloc.is_available();

  rsx! {
    section { class: "page",
      div { class: "page-heading",
        div {
          p { class: "eyebrow", "偏好" }
          h1 { "设置" }
        }
      }

      div { class: "settings-section",
        h2 { "本地存储" }
        div { class: "setting-row",
          div {
            p { class: "setting-title", "RocksDB" }
            p { class: "setting-detail", "{state.tasks.len()} 条记录" }
          }
          span {
            class: if storage_available { "setting-badge" } else { "setting-badge error" },
            if storage_available { "运行中" } else { "不可用" }
          }
        }
        code { class: "database-path", {database_path} }
      }

      div { class: "settings-section danger-zone",
        div {
          h2 { "数据整理" }
          p { "移除所有已完成任务" }
        }
        button {
          class: "secondary-button",
          disabled: state.completed_count() == 0,
          onclick: {
            let mut bloc = bloc.clone();
            move |_| bloc.dispatch(TaskEvent::ClearCompleted)
          },
          "清理已完成"
        }
      }
    }
  }
}

#[component]
fn FilterButton(label: &'static str, filter: TaskFilter, selected: bool) -> Element {
  let mut bloc = use_task_bloc();
  rsx! {
    button {
      class: if selected { "filter-button selected" } else { "filter-button" },
      aria_pressed: selected,
      onclick: move |_| bloc.dispatch(TaskEvent::SetFilter(filter)),
      {label}
    }
  }
}

#[component]
fn TaskRow(task: Task) -> Element {
  let bloc = use_task_bloc();
  let task_id = task.id;
  let completion_label = if task.completed {
    "标记为进行中"
  } else {
    "标记为已完成"
  };

  rsx! {
    article { class: if task.completed { "task-row completed" } else { "task-row" },
      label { class: "check-control", title: completion_label,
        input {
          r#type: "checkbox",
          checked: task.completed,
          aria_label: "{completion_label}: {task.title}",
          onchange: {
            let mut bloc = bloc.clone();
            move |_| bloc.dispatch(TaskEvent::Toggle(task_id))
          },
        }
        span { class: "checkmark", aria_hidden: "true", "✓" }
      }
      p { class: "task-title", "{task.title}" }
      button {
        class: "delete-button",
        title: "删除任务",
        aria_label: "删除任务: {task.title}",
        onclick: {
          let mut bloc = bloc.clone();
          move |_| bloc.dispatch(TaskEvent::Delete(task_id))
        },
        span { aria_hidden: "true", "×" }
      }
    }
  }
}

#[component]
fn ErrorBanner(message: String) -> Element {
  let mut bloc = use_task_bloc();
  rsx! {
    div { class: "error-banner", role: "alert",
      p { {message} }
      button {
        title: "关闭",
        aria_label: "关闭错误提示",
        onclick: move |_| bloc.dispatch(TaskEvent::DismissError),
        "×"
      }
    }
  }
}

#[component]
fn Metric(value: usize, label: &'static str, tone: &'static str) -> Element {
  rsx! {
    div { class: "metric {tone}",
      strong { "{value}" }
      span { {label} }
    }
  }
}
