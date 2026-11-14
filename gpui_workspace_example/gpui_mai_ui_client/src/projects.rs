use std::{
  collections::HashSet,
  fs,
  io::Write,
  path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
  grounding::GroundingSession,
  session::{Session, Settings},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
  pub id: u64,
  pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Catalog {
  version: u32,
  active: u64,
  projects: Vec<ProjectInfo>,
}

pub struct ProjectData {
  pub path: PathBuf,
  pub session: Session,
  pub grounding: GroundingSession,
}

pub struct ProjectStore {
  legacy_path: PathBuf,
  catalog_path: PathBuf,
  catalog: Catalog,
}

fn name_key(name: &str) -> Result<String> {
  ensure!(!name.trim().is_empty(), "工程名称不能为空");
  ensure!(name.chars().count() <= 80, "工程名称最多 80 个字符");
  ensure!(
    !name.chars().any(char::is_control),
    "工程名称不能包含换行或控制字符"
  );
  Ok(name.trim().to_lowercase())
}

impl Catalog {
  fn validate(&self) -> Result<()> {
    ensure!(self.version == 1, "不支持的工程索引版本");
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for project in &self.projects {
      ensure!(ids.insert(project.id), "工程编号重复");
      ensure!(names.insert(name_key(&project.name)?), "工程名称重复");
    }
    ensure!(ids.contains(&0), "工程索引缺少默认工程");
    ensure!(ids.contains(&self.active), "当前工程编号无效");
    Ok(())
  }
}

impl ProjectStore {
  pub fn open(path: &Path) -> Result<Self> {
    let legacy_path = if path.is_absolute() {
      path.to_owned()
    } else {
      std::env::current_dir()?.join(path)
    };
    let catalog_path = legacy_path.with_extension("projects.json");
    let catalog = match fs::read(&catalog_path) {
      Ok(bytes) => serde_json::from_slice::<Catalog>(&bytes)
        .context("工程索引损坏；请备份后修复，不会覆盖原索引")?,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
        let session = Session::load(&legacy_path)?;
        let grounding_path = legacy_path.with_extension("grounding.json");
        let grounding = GroundingSession::load(&grounding_path, &session.settings)?;
        if !legacy_path.exists() {
          session.save(&legacy_path)?;
        }
        if !grounding_path.exists() {
          grounding.save(&grounding_path)?;
        }
        let store = Self {
          legacy_path,
          catalog_path,
          catalog: Catalog {
            version: 1,
            active: 0,
            projects: vec![ProjectInfo {
              id: 0,
              name: "默认工程".into(),
            }],
          },
        };
        store.write_catalog(&store.catalog)?;
        return Ok(store);
      }
      Err(error) => return Err(error).context("读取工程索引失败"),
    };
    catalog.validate()?;
    Ok(Self {
      legacy_path,
      catalog_path,
      catalog,
    })
  }

  pub fn catalog_path(&self) -> &Path {
    &self.catalog_path
  }

  pub fn projects(&self) -> &[ProjectInfo] {
    &self.catalog.projects
  }

  pub fn active(&self) -> &ProjectInfo {
    self
      .catalog
      .projects
      .iter()
      .find(|project| project.id == self.catalog.active)
      .unwrap_or(&self.catalog.projects[0])
  }

  fn project_path(&self, id: u64) -> PathBuf {
    if id == 0 {
      self.legacy_path.clone()
    } else {
      self
        .legacy_path
        .with_extension("projects")
        .join(id.to_string())
        .join("session.json")
    }
  }

  fn load(&self, id: u64) -> Result<ProjectData> {
    ensure!(
      self.catalog.projects.iter().any(|project| project.id == id),
      "工程不存在"
    );
    let path = self.project_path(id);
    let grounding_path = path.with_extension("grounding.json");
    ensure!(
      path.is_file() && grounding_path.is_file(),
      "工程会话文件缺失，请恢复文件后重试；不会用空工程覆盖"
    );
    let session = Session::load(&path).context("读取工程设计会话失败")?;
    let grounding =
      GroundingSession::load(&grounding_path, &session.settings).context("读取工程定位会话失败")?;
    Ok(ProjectData {
      path,
      session,
      grounding,
    })
  }

  pub fn load_active(&self) -> Result<ProjectData> {
    self.load(self.catalog.active)
  }

  fn write_catalog(&self, catalog: &Catalog) -> Result<()> {
    catalog.validate()?;
    let temporary = self.catalog_path.with_extension("json.tmp");
    let mut file = fs::File::create(&temporary).context("创建工程索引临时文件失败")?;
    file.write_all(&serde_json::to_vec_pretty(catalog)?)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, &self.catalog_path).context("保存工程索引失败")
  }

  pub fn select(&mut self, id: u64) -> Result<ProjectData> {
    let data = self.load(id)?;
    let mut catalog = self.catalog.clone();
    catalog.active = id;
    self.write_catalog(&catalog)?;
    self.catalog = catalog;
    Ok(data)
  }

  pub fn create(
    &mut self,
    name: &str,
    settings: Settings,
    grounding_settings: Settings,
  ) -> Result<ProjectData> {
    let name = name.trim();
    let key = name_key(name)?;
    ensure!(
      !self
        .catalog
        .projects
        .iter()
        .any(|project| project.name.trim().to_lowercase() == key),
      "已有同名工程，请使用其他名称"
    );
    settings.validate()?;
    grounding_settings.validate()?;
    let mut id = self
      .catalog
      .projects
      .iter()
      .map(|project| project.id)
      .max()
      .unwrap_or(0);
    loop {
      id = id.checked_add(1).context("工程编号已耗尽")?;
      if !self
        .project_path(id)
        .parent()
        .context("工程目录无效")?
        .exists()
      {
        break;
      }
    }
    let path = self.project_path(id);
    let parent = path.parent().context("工程目录无效")?;
    fs::create_dir_all(parent.parent().context("工程根目录无效")?)?;
    fs::create_dir(parent).context("创建工程目录失败")?;
    let session = Session {
      settings,
      draft: String::new(),
      ..Session::default()
    };
    let grounding = GroundingSession::new(grounding_settings);
    session.save(&path)?;
    grounding.save(&path.with_extension("grounding.json"))?;
    let mut catalog = self.catalog.clone();
    catalog.projects.push(ProjectInfo {
      id,
      name: name.to_owned(),
    });
    catalog.active = id;
    self.write_catalog(&catalog)?;
    self.catalog = catalog;
    Ok(ProjectData {
      path,
      session,
      grounding,
    })
  }
}

pub fn prompt_report(session: &Session, grounding: &GroundingSession) -> String {
  let mut sections = vec![format!("【界面设计 · 当前草稿】\n{}", session.draft)];
  sections.push(format!("【收藏提示词 · {} 条】", session.prompts.len()));
  for (index, prompt) in session.prompts.iter().enumerate() {
    sections.push(format!("收藏 {}\n{prompt}", index + 1));
  }
  sections.push(format!(
    "【版本提示词 · {} 条（含所有分支）】",
    session.revisions.len()
  ));
  for (index, revision) in session.revisions.iter().enumerate() {
    let parent = revision
      .parent
      .map(|parent| format!("V{}", parent + 1))
      .unwrap_or_else(|| "空白".into());
    sections.push(format!(
      "V{} · 基于 {parent}\n{}",
      index + 1,
      revision.prompt
    ));
  }
  sections.push(format!("【截图定位 · 当前指令草稿】\n{}", grounding.draft));
  sections.push(format!("【截图定位指令 · {} 条】", grounding.steps().len()));
  for (index, step) in grounding.steps().iter().enumerate() {
    sections.push(format!("定位 {}\n{}", index + 1, step.instruction));
  }
  sections.join("\n\n")
}
