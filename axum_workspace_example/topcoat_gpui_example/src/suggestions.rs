//! Bounded, deterministic autocomplete using the same state as desktop/browser writes.
use std::collections::HashSet;

use topcoat_gpui_protocol::{
  DemoSnapshot, Suggestion, SuggestionKind, SuggestionSource, Suggestions,
};

pub fn search(kind: SuggestionKind, query: &str, state: &DemoSnapshot) -> Suggestions {
  let query = query.trim().to_owned();
  let needle = query.to_lowercase();
  if needle.is_empty() {
    return Suggestions {
      query,
      items: vec![],
    };
  }
  let seeds: &[(&str, &str, &str)] = match kind {
    SuggestionKind::Todo => &[
      (
        "学习 Rust 异步编程",
        "xuexi rust yibu biancheng xxrust",
        "学习 · Rust / Tokio",
      ),
      (
        "学习抗量子加密",
        "xuexi kangliangzi jiami xxklzjm",
        "学习 · 安全通信",
      ),
      (
        "学习 WebSocket 实时通信",
        "xuexi websocket shishi tongxin xxws",
        "学习 · 实时协作",
      ),
      (
        "开发实时搜索补全",
        "kaifa shishi sousuo buquan kfssbq",
        "开发 · 交互体验",
      ),
      (
        "开发 GPUI 桌面界面",
        "kaifa gpui zhuomian jiemian kfgpui",
        "开发 · 原生桌面",
      ),
      (
        "检查加密通信测试",
        "jiancha jiami tongxin ceshi jcjmtx",
        "测试 · 双端联调",
      ),
      (
        "整理项目文档",
        "zhengli xiangmu wendang zlxmwd",
        "计划 · 文档",
      ),
      (
        "准备每周工作计划",
        "zhunbei meizhou gongzuo jihua zbmzgz",
        "计划 · 每周回顾",
      ),
      ("Review pull requests", "review pr", "开发 · 代码审查"),
    ],
    SuggestionKind::Profile => &[
      ("张小明", "zhang xiaoming zxm", "示例联系人 · 产品"),
      ("张晓雨", "zhang xiaoyu zxy", "示例联系人 · 设计"),
      ("李华", "li hua lh", "示例联系人 · 开发"),
      ("王晨", "wang chen wc", "示例联系人 · 测试"),
      ("陈星", "chen xing cx", "示例联系人 · 研发"),
      ("Alice", "alice", "示例联系人 · Design"),
      ("Alex", "alex", "示例联系人 · Engineering"),
    ],
  };
  let mut rows: Vec<(u8, u8, usize, Suggestion)> = Vec::new();
  let mut seen = HashSet::new();
  let mut add = |value: &str, aliases: &str, detail: &str, shared: bool| {
    let lower = value.to_lowercase();
    let score = if lower.starts_with(&needle) {
      0
    } else if lower.contains(&needle) {
      1
    } else if aliases.contains(&needle) {
      2
    } else {
      return;
    };
    if !seen.insert(lower) {
      return;
    }
    rows.push((
      score,
      if shared { 0 } else { 1 },
      rows.len(),
      Suggestion {
        value: value.into(),
        detail: detail.into(),
        source: if shared {
          SuggestionSource::Shared
        } else {
          SuggestionSource::Suggested
        },
      },
    ));
  };
  match kind {
    SuggestionKind::Todo => {
      for todo in state.todos.iter().rev() {
        add(
          &todo.title,
          "",
          if todo.done {
            "共享待办 · 已完成"
          } else {
            "共享待办 · 未完成"
          },
          true,
        );
      }
    }
    SuggestionKind::Profile => {
      for profile in &state.profiles {
        add(&profile.username, "", "两端共享联系人", true);
      }
    }
  }
  for (value, aliases, detail) in seeds {
    add(value, aliases, detail, false);
  }
  rows.sort_by_key(|(score, source, order, _)| (*score, *source, *order));
  Suggestions {
    query,
    items: rows
      .into_iter()
      .take(6)
      .map(|(_, _, _, item)| item)
      .collect(),
  }
}
#[cfg(test)]
mod tests {
  use topcoat_gpui_protocol::{Profile, Todo};

  use super::*;
  #[test]
  fn matches_chinese_aliases_and_shared_records_without_duplicates() {
    let mut state = DemoSnapshot::default();
    assert!(search(SuggestionKind::Todo, "学", &state).items.len() >= 3);
    assert_eq!(
      search(SuggestionKind::Profile, "zxm", &state).items[0].value,
      "张小明"
    );
    assert_eq!(
      search(SuggestionKind::Profile, " ALI ", &state).items[0].value,
      "Alice"
    );
    for id in 0 .. 12 {
      state.todos.push(Todo {
        id,
        title: format!("学习共享任务 {id}"),
        done: false,
      });
    }
    state.todos.push(Todo {
      id: 12,
      title: "学习 Rust 异步编程".into(),
      done: false,
    });
    let response = search(SuggestionKind::Todo, "学", &state);
    assert_eq!(response.items.len(), 6);
    assert_eq!(response.items[0].value, "学习 Rust 异步编程");
    assert_eq!(response.items[0].source, SuggestionSource::Shared);
    state.profiles.push(Profile {
      username: "Alice".into(),
      age: 22,
    });
    assert_eq!(
      search(SuggestionKind::Profile, "ali", &state).items.len(),
      1
    );
    assert!(search(SuggestionKind::Todo, "   ", &state).items.is_empty());
    assert!(
      search(SuggestionKind::Todo, "不存在xyz", &state)
        .items
        .is_empty()
    );
  }
}
