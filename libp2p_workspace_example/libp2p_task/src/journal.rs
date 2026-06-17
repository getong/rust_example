use std::{path::Path, sync::Arc};

use anyhow::Context;
use rocksdb::{DB, IteratorMode, Options};

use crate::domain::TaskUnderstanding;

#[derive(Clone)]
pub(crate) struct ConsensusTaskJournal {
  db: Arc<DB>,
}

impl ConsensusTaskJournal {
  pub(crate) fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
    let path = path.as_ref();
    let mut options = Options::default();
    options.create_if_missing(true);
    let db = DB::open(&options, path)
      .with_context(|| format!("open task journal rocksdb at {}", path.display()))?;
    Ok(Self { db: Arc::new(db) })
  }

  pub(crate) async fn append(&self, understanding: TaskUnderstanding) -> anyhow::Result<()> {
    let db = self.db.clone();
    tokio::task::spawn_blocking(move || {
      let key = journal_key(&understanding);
      let value = serde_json::to_vec(&understanding).context("encode task understanding")?;
      db.put(key, value).context("write task understanding")
    })
    .await
    .context("join rocksdb append task")?
  }

  pub(crate) async fn list(&self) -> anyhow::Result<Vec<TaskUnderstanding>> {
    let db = self.db.clone();
    tokio::task::spawn_blocking(move || {
      let mut rows = Vec::new();
      for item in db.iterator(IteratorMode::Start) {
        let (_, value) = item.context("iterate task journal")?;
        let understanding = serde_json::from_slice(&value).context("decode task understanding")?;
        rows.push(understanding);
      }
      Ok(rows)
    })
    .await
    .context("join rocksdb list task")?
  }
}

fn journal_key(understanding: &TaskUnderstanding) -> Vec<u8> {
  format!(
    "{:020}:{}:{:?}:{}",
    understanding.at_ms, understanding.task_id, understanding.stage, understanding.node
  )
  .into_bytes()
}
