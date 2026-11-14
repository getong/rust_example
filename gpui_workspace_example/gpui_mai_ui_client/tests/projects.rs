use std::{
  fs,
  io::Cursor,
  path::PathBuf,
  time::{SystemTime, UNIX_EPOCH},
};

use gpui_mai_ui_client::{
  grounding::{GroundingSession, Screenshot},
  projects::{ProjectStore, prompt_report},
  session::{Session, Settings},
};
use image::{DynamicImage, ImageFormat};
use serde_json::{Value, json};

const PAGE: &str =
  "<!doctype html><html><head><style>body{color:blue}</style></head><body>工程</body></html>";

struct Directory(PathBuf);

impl Directory {
  fn new() -> Self {
    let root = std::env::temp_dir().join(format!(
      "mai-project-test-{}-{}",
      std::process::id(),
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    Self(root)
  }

  fn session(&self) -> PathBuf {
    self.0.join("session.json")
  }
}

impl Drop for Directory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn add_grounding(session: &mut GroundingSession, instruction: &str) {
  let mut bytes = Cursor::new(Vec::new());
  DynamicImage::new_rgb8(8, 8)
    .write_to(&mut bytes, ImageFormat::Png)
    .unwrap();
  session.import(Screenshot::from_bytes("测试截图.png".into(), bytes.get_ref()).unwrap());
  session
    .commit(
      instruction.into(),
      "<answer>{\"coordinate\":[500,200]}</answer>".into(),
      Some(12),
    )
    .unwrap();
}

#[test]
fn legacy_session_is_registered_without_rewriting_or_losing_history() {
  let directory = Directory::new();
  let path = directory.session();
  let mut session = Session {
    draft: "原工程未提交的中文草稿".into(),
    ..Session::default()
  };
  session
    .commit("已有版本".into(), PAGE.into(), None)
    .unwrap();
  session.save(&path).unwrap();
  let mut grounding = GroundingSession::new(session.settings.clone());
  add_grounding(&mut grounding, "定位设置按钮");
  grounding
    .save(&path.with_extension("grounding.json"))
    .unwrap();
  let original = fs::read(&path).unwrap();
  let original_grounding = fs::read(path.with_extension("grounding.json")).unwrap();
  let store = ProjectStore::open(&path).unwrap();
  assert_eq!(store.projects().len(), 1);
  assert_eq!(store.active().name, "默认工程");
  let loaded = store.load_active().unwrap();
  assert_eq!(loaded.path, path);
  assert_eq!(loaded.session.draft, "原工程未提交的中文草稿");
  assert_eq!(loaded.session.revisions.len(), 1);
  assert_eq!(
    loaded.grounding.selected().unwrap().instruction,
    "定位设置按钮"
  );
  assert_eq!(fs::read(&path).unwrap(), original);
  assert_eq!(
    fs::read(path.with_extension("grounding.json")).unwrap(),
    original_grounding
  );
}

#[test]
fn new_projects_start_empty_and_inherit_only_model_settings() {
  let directory = Directory::new();
  let mut store = ProjectStore::open(&directory.session()).unwrap();
  let settings = Settings {
    model: "code-model".into(),
    ..Settings::default()
  };
  let grounding_settings = Settings {
    model: "vision-model".into(),
    ..Settings::default()
  };
  let data = store
    .create(" 中文播放器 ", settings, grounding_settings)
    .unwrap();
  assert_eq!(store.active().name, "中文播放器");
  assert_eq!(store.projects().len(), 2);
  assert_eq!(data.session.settings.model, "code-model");
  assert_eq!(data.grounding.settings.model, "vision-model");
  assert!(data.session.draft.is_empty());
  assert!(data.session.prompts.is_empty());
  assert!(data.session.revisions.is_empty());
  assert!(data.grounding.draft.is_empty());
  assert!(data.grounding.steps().is_empty());
  assert!(data.path.is_file());
  assert!(data.path.with_extension("grounding.json").is_file());
  let restarted = ProjectStore::open(&directory.session()).unwrap();
  assert_eq!(restarted.active().id, store.active().id);
  assert_eq!(
    restarted.load_active().unwrap().session.settings.model,
    "code-model"
  );
}

#[test]
fn switching_projects_keeps_drafts_prompts_branches_and_screenshots_isolated() {
  let directory = Directory::new();
  let mut store = ProjectStore::open(&directory.session()).unwrap();
  let mut first = store
    .create("工程一", Settings::default(), Settings::default())
    .unwrap();
  let first_id = store.active().id;
  first.session.draft = "工程一的草稿".into();
  first
    .session
    .commit("工程一根提示词".into(), PAGE.into(), Some(10))
    .unwrap();
  first
    .session
    .commit("工程一第二版".into(), PAGE.into(), None)
    .unwrap();
  first.session.back();
  first
    .session
    .commit("工程一分支".into(), PAGE.into(), None)
    .unwrap();
  first.session.save(&first.path).unwrap();
  add_grounding(&mut first.grounding, "工程一定位");
  first
    .grounding
    .save(&first.path.with_extension("grounding.json"))
    .unwrap();
  let mut second = store
    .create("工程二", Settings::default(), Settings::default())
    .unwrap();
  let second_id = store.active().id;
  second.session.draft = "工程二的草稿".into();
  second.session.remember_prompt("工程二收藏");
  second.session.save(&second.path).unwrap();
  let restored_first = store.select(first_id).unwrap();
  assert_eq!(restored_first.session.draft, "工程一的草稿");
  assert_eq!(restored_first.session.revisions.len(), 3);
  assert_eq!(restored_first.session.active, Some(2));
  assert_eq!(restored_first.session.revisions[2].parent, Some(0));
  assert!(
    !restored_first
      .session
      .prompts
      .iter()
      .any(|prompt| prompt == "工程二收藏")
  );
  assert_eq!(
    restored_first.grounding.steps()[0].instruction,
    "工程一定位"
  );
  let restored_second = store.select(second_id).unwrap();
  assert_eq!(restored_second.session.draft, "工程二的草稿");
  assert_eq!(restored_second.session.prompts, ["工程二收藏"]);
  assert!(restored_second.session.revisions.is_empty());
  assert!(restored_second.grounding.screenshot().is_none());
  store.select(0).unwrap();
  assert_eq!(
    ProjectStore::open(&directory.session())
      .unwrap()
      .active()
      .id,
    0
  );
}

#[test]
fn invalid_or_duplicate_names_do_not_create_projects_or_change_selection() {
  let directory = Directory::new();
  let mut store = ProjectStore::open(&directory.session()).unwrap();
  store
    .create("Demo", Settings::default(), Settings::default())
    .unwrap();
  let active = store.active().id;
  let too_long = "中".repeat(81);
  for name in [
    "",
    "  ",
    " demo ",
    "默认工程",
    "含\n换行",
    "含\t制表",
    &too_long,
  ] {
    assert!(
      store
        .create(name, Settings::default(), Settings::default())
        .is_err(),
      "{name}"
    );
    assert_eq!(store.active().id, active);
    assert_eq!(store.projects().len(), 2);
  }
  let invalid = Settings {
    model: String::new(),
    ..Settings::default()
  };
  assert!(
    store
      .create("配置错误", invalid, Settings::default())
      .is_err()
  );
  assert_eq!(store.projects().len(), 2);
}

#[test]
fn project_names_are_not_used_as_filesystem_paths() {
  let directory = Directory::new();
  let mut store = ProjectStore::open(&directory.session()).unwrap();
  let project = store
    .create("../../不会逃逸", Settings::default(), Settings::default())
    .unwrap();
  assert_eq!(
    project.path,
    directory.0.join("session.projects/1/session.json")
  );
}

#[test]
fn failed_or_corrupted_project_selection_preserves_active_catalog_and_files() {
  let directory = Directory::new();
  let mut store = ProjectStore::open(&directory.session()).unwrap();
  let first = store
    .create("正常工程", Settings::default(), Settings::default())
    .unwrap();
  let first_id = store.active().id;
  let second = store
    .create("损坏工程", Settings::default(), Settings::default())
    .unwrap();
  let second_id = store.active().id;
  store.select(first_id).unwrap();
  let catalog = fs::read(store.catalog_path()).unwrap();
  fs::write(&second.path, b"not json").unwrap();
  assert!(store.select(second_id).is_err());
  assert!(store.select(999).is_err());
  assert_eq!(store.active().id, first_id);
  assert_eq!(fs::read(store.catalog_path()).unwrap(), catalog);
  assert_eq!(fs::read(&second.path).unwrap(), b"not json");
  fs::remove_file(first.path.with_extension("grounding.json")).unwrap();
  assert!(store.load_active().is_err());
  assert!(!first.path.with_extension("grounding.json").exists());
}

#[test]
fn failed_index_write_does_not_switch_project_and_retry_skips_orphan_directory() {
  let directory = Directory::new();
  let mut store = ProjectStore::open(&directory.session()).unwrap();
  let catalog = fs::read(store.catalog_path()).unwrap();
  let temporary = store.catalog_path().with_extension("json.tmp");
  fs::create_dir(&temporary).unwrap();
  assert!(
    store
      .create("待创建工程", Settings::default(), Settings::default())
      .is_err()
  );
  assert_eq!(store.active().id, 0);
  assert_eq!(store.projects().len(), 1);
  assert_eq!(fs::read(store.catalog_path()).unwrap(), catalog);
  fs::write(
    directory.0.join("session.projects/1/session.json"),
    b"orphan-do-not-overwrite",
  )
  .unwrap();
  fs::remove_dir(temporary).unwrap();
  let retry = store
    .create("待创建工程", Settings::default(), Settings::default())
    .unwrap();
  assert_eq!(store.active().id, 2);
  assert_eq!(
    fs::read(directory.0.join("session.projects/1/session.json")).unwrap(),
    b"orphan-do-not-overwrite"
  );
  assert_ne!(retry.path, directory.session());
}

#[test]
fn invalid_catalog_is_rejected_without_overwriting_it_or_legacy_data() {
  let directory = Directory::new();
  let store = ProjectStore::open(&directory.session()).unwrap();
  let original: Value = serde_json::from_slice(&fs::read(store.catalog_path()).unwrap()).unwrap();
  let legacy = fs::read(directory.session()).unwrap();
  for (pointer, value) in [
    ("/version", json!(999)),
    ("/active", json!(999)),
    ("/projects", json!([])),
    (
      "/projects",
      json!([{"id":0,"name":"一"},{"id":0,"name":"二"}]),
    ),
    (
      "/projects",
      json!([{"id":0,"name":"一"},{"id":1,"name":"一"}]),
    ),
  ] {
    let mut invalid = original.clone();
    *invalid.pointer_mut(pointer).unwrap() = value;
    let bytes = serde_json::to_vec(&invalid).unwrap();
    fs::write(store.catalog_path(), &bytes).unwrap();
    assert!(ProjectStore::open(&directory.session()).is_err());
    assert_eq!(fs::read(store.catalog_path()).unwrap(), bytes);
    assert_eq!(fs::read(directory.session()).unwrap(), legacy);
  }
}

#[test]
fn prompt_report_contains_full_drafts_favorites_all_branches_and_grounding_instructions() {
  let mut session = Session {
    draft: "尚未提交的草稿".into(),
    ..Session::default()
  };
  let long_prompt = "完整中文提示词".repeat(100);
  session.remember_prompt(&long_prompt);
  session.commit("根版本".into(), PAGE.into(), None).unwrap();
  session.commit("旧分支".into(), PAGE.into(), None).unwrap();
  session.back();
  session.commit("新分支".into(), PAGE.into(), None).unwrap();
  let mut grounding = GroundingSession::new(Settings::default());
  grounding.draft = "尚未提交的定位指令".into();
  add_grounding(&mut grounding, "已执行的定位指令");
  let report = prompt_report(&session, &grounding);
  for expected in [
    &long_prompt,
    "尚未提交的草稿",
    "根版本",
    "旧分支",
    "新分支",
    "V3 · 基于 V1",
    "尚未提交的定位指令",
    "已执行的定位指令",
  ] {
    assert!(report.contains(expected), "{expected}");
  }
  assert!(!report.contains(PAGE));
}
