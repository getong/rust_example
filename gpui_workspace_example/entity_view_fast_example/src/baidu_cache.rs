use std::path::PathBuf;

use anyhow::{Context as _, Result, ensure};

use crate::baidu_tab::{BOARD_URL, Board};

/// A single atomic snapshot per source. UI data is always read back from this table.
#[derive(Clone)]
pub(crate) struct BaiduCache {
  path: PathBuf,
  #[cfg(test)]
  _directory: Option<std::sync::Arc<tempfile::TempDir>>,
}
pub(crate) struct CachedBoard {
  pub boards: Vec<Board>,
  pub fetched_at: i64,
}
impl CachedBoard {
  pub fn fresh_at(&self, now: i64) -> bool {
    (0 .. 300).contains(&(now - self.fetched_at))
  }
}
impl Default for BaiduCache {
  fn default() -> Self {
    #[cfg(not(test))]
    {
      Self {
        path: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/baidu.db"),
      }
    }
    #[cfg(test)]
    {
      let directory = std::sync::Arc::new(tempfile::tempdir().expect("test cache directory"));
      Self {
        path: directory.path().join("baidu.db"),
        _directory: Some(directory),
      }
    }
  }
}
impl BaiduCache {
  async fn connect(&self) -> Result<turso::Connection> {
    if let Some(parent) = self.path.parent() {
      std::fs::create_dir_all(parent).context("创建缓存目录失败")?;
    }
    let db = turso::Builder::new_local(self.path.to_str().context("数据库路径不是 UTF-8")?)
      .build()
      .await
      .context("打开 Turso 缓存失败")?;
    let conn = db.connect().context("连接 Turso 缓存失败")?;
    conn
      .execute(
        "CREATE TABLE IF NOT EXISTS baidu_cache (source TEXT PRIMARY KEY, payload TEXT NOT NULL, \
         fetched_at INTEGER NOT NULL)",
        (),
      )
      .await
      .context("初始化缓存表失败")?;
    Ok(conn)
  }
  async fn read(conn: &turso::Connection) -> Result<Option<CachedBoard>> {
    let mut rows = conn
      .query(
        "SELECT payload, fetched_at FROM baidu_cache WHERE source = ?1",
        (BOARD_URL,),
      )
      .await?;
    if let Some(row) = rows.next().await? {
      let json: String = row.get(0)?;
      let boards: Vec<Board> = serde_json::from_str(&json).context("缓存 JSON 解码失败")?;
      ensure!(!boards.is_empty(), "缓存榜单为空");
      Ok(Some(CachedBoard {
        boards,
        fetched_at: row.get(1)?,
      }))
    } else {
      Ok(None)
    }
  }
  pub async fn load(&self) -> Result<Option<CachedBoard>> {
    let conn = self.connect().await?;
    Self::read(&conn).await.context("读取 Turso 缓存失败")
  }
  pub async fn save_and_load(&self, boards: &[Board], fetched_at: i64) -> Result<CachedBoard> {
    ensure!(!boards.is_empty(), "拒绝用空榜单覆盖缓存");
    let json = serde_json::to_string(boards)?;
    let conn = self.connect().await?;
    // One statement commits the entire snapshot: readers never see a half-written board.
    conn
      .execute(
        "INSERT INTO baidu_cache(source,payload,fetched_at) VALUES(?1,?2,?3) ON CONFLICT(source) \
         DO UPDATE SET payload=excluded.payload, fetched_at=excluded.fetched_at",
        (BOARD_URL, json, fetched_at),
      )
      .await
      .context("写入 Turso 缓存失败")?;
    Self::read(&conn).await?.context("写入后未能读回缓存")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn boards(title: &str) -> Vec<Board> {
    serde_json::from_value(
      serde_json::json!([{"text":"热搜榜","content":[{"word":title,"hotScore":"100"}]}]),
    )
    .unwrap()
  }

  #[test]
  fn persists_reads_back_and_atomically_replaces_snapshot() {
    futures::executor::block_on(async {
      let cache = BaiduCache::default();
      assert!(cache.load().await.unwrap().is_none());
      let first = cache.save_and_load(&boards("first"), 1000).await.unwrap();
      assert_eq!(first.fetched_at, 1000);
      assert!(first.fresh_at(1299));
      assert!(!first.fresh_at(1300));
      assert!(!first.fresh_at(999));
      let reopened = cache.clone();
      drop(cache);
      assert!(reopened.path.exists());
      let saved = reopened.load().await.unwrap().unwrap();
      assert_eq!(
        serde_json::to_value(saved.boards).unwrap()[0]["content"][0]["word"],
        "first"
      );
      reopened
        .save_and_load(&boards("second"), 2000)
        .await
        .unwrap();
      assert!(reopened.save_and_load(&[], 3000).await.is_err());
      let latest = reopened.load().await.unwrap().unwrap();
      assert_eq!(latest.fetched_at, 2000);
      assert_eq!(
        serde_json::to_value(latest.boards).unwrap()[0]["content"][0]["word"],
        "second"
      );
    });
  }
}
