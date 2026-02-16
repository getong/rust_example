use std::{path::PathBuf, sync::Mutex};

use deno_core::url::Url;
use deno_runtime::{
  code_cache::{self, CodeCacheType},
  deno_webstorage::rusqlite::{self, Connection, OptionalExtension, params},
};

/// SQLite-backed V8 code cache. Stores compiled bytecode per (specifier, type)
/// pair so that modules don't need to be recompiled from source on every startup.
///
/// Modelled after the `embed_deno` and `deno` CLI implementations.
pub struct CodeCache {
  conn: Mutex<Connection>,
}

impl std::fmt::Debug for CodeCache {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CodeCache").finish()
  }
}

impl CodeCache {
  /// Open (or create) the code cache database at the given path.
  pub fn new(path: &PathBuf) -> Result<Self, rusqlite::Error> {
    if let Some(parent) = path.parent() {
      let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(
      "PRAGMA journal_mode=WAL;PRAGMA synchronous=NORMAL;PRAGMA temp_store=memory;PRAGMA \
       page_size=4096;PRAGMA mmap_size=6000000;CREATE TABLE IF NOT EXISTS codecache (specifier \
       TEXT NOT NULL,type INTEGER NOT NULL,source_hash INTEGER NOT NULL,data BLOB NOT \
       NULL,PRIMARY KEY (specifier, type));",
    )?;
    Ok(Self {
      conn: Mutex::new(conn),
    })
  }

  /// Open an in-memory code cache (useful for testing or when disk is unavailable).
  #[allow(dead_code)]
  pub fn in_memory() -> Result<Self, rusqlite::Error> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
      "CREATE TABLE IF NOT EXISTS codecache (specifier TEXT NOT NULL,type INTEGER NOT \
       NULL,source_hash INTEGER NOT NULL,data BLOB NOT NULL,PRIMARY KEY (specifier, type));",
    )?;
    Ok(Self {
      conn: Mutex::new(conn),
    })
  }
}

fn type_to_i64(t: CodeCacheType) -> i64 {
  match t {
    CodeCacheType::Script => 0,
    CodeCacheType::EsModule => 1,
  }
}

impl code_cache::CodeCache for CodeCache {
  fn get_sync(
    &self,
    specifier: &Url,
    code_cache_type: CodeCacheType,
    source_hash: u64,
  ) -> Option<Vec<u8>> {
    let conn = match self.conn.lock() {
      Ok(c) => c,
      Err(_) => return None,
    };
    conn
      .query_row(
        "SELECT data FROM codecache WHERE specifier=?1 AND type=?2 AND source_hash=?3 LIMIT 1",
        params![
          specifier.as_str(),
          type_to_i64(code_cache_type),
          source_hash as i64,
        ],
        |row| row.get(0),
      )
      .optional()
      .ok()
      .flatten()
  }

  fn set_sync(
    &self,
    specifier: Url,
    code_cache_type: CodeCacheType,
    source_hash: u64,
    data: &[u8],
  ) {
    let conn = match self.conn.lock() {
      Ok(c) => c,
      Err(_) => return,
    };
    let _ = conn.execute(
      "INSERT OR REPLACE INTO codecache (specifier, type, source_hash, data) VALUES (?1, ?2, ?3, \
       ?4)",
      params![
        specifier.as_str(),
        type_to_i64(code_cache_type),
        source_hash as i64,
        data,
      ],
    );
  }
}
