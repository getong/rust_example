use std::{path::PathBuf, sync::Mutex};

use deno_runtime::deno_webstorage::rusqlite::{self, Connection, OptionalExtension, params};

/// SQLite-backed transpile cache. Stores transpiled JavaScript output for
/// TypeScript/JSX sources keyed by (specifier, source_hash) to avoid
/// re-transpilation across runs.
pub struct TranspileCache {
  conn: Mutex<Connection>,
}

impl std::fmt::Debug for TranspileCache {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TranspileCache").finish()
  }
}

impl TranspileCache {
  /// Open (or create) the transpile cache database at the given path.
  pub fn new(path: &PathBuf) -> Result<Self, rusqlite::Error> {
    if let Some(parent) = path.parent() {
      let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(
      "PRAGMA journal_mode=WAL;PRAGMA synchronous=NORMAL;PRAGMA temp_store=memory;PRAGMA \
       page_size=4096;CREATE TABLE IF NOT EXISTS transpilecache (specifier TEXT NOT NULL PRIMARY \
       KEY,source_hash INTEGER NOT NULL,transpiled_code TEXT NOT NULL,source_map BLOB);",
    )?;
    Ok(Self {
      conn: Mutex::new(conn),
    })
  }

  /// Try to get a cached transpile result.  Returns `(transpiled_code, Option<source_map_bytes>)`.
  pub fn get(&self, specifier: &str, source_hash: u64) -> Option<(String, Option<Vec<u8>>)> {
    let conn = self.conn.lock().ok()?;
    conn
      .query_row(
        "SELECT transpiled_code, source_map FROM transpilecache WHERE specifier=?1 AND \
         source_hash=?2 LIMIT 1",
        params![specifier, source_hash as i64],
        |row| {
          let code: String = row.get(0)?;
          let source_map: Option<Vec<u8>> = row.get(1)?;
          Ok((code, source_map))
        },
      )
      .optional()
      .ok()
      .flatten()
  }

  /// Store a transpiled result.
  pub fn set(
    &self,
    specifier: &str,
    source_hash: u64,
    transpiled_code: &str,
    source_map: Option<&[u8]>,
  ) {
    let Ok(conn) = self.conn.lock() else {
      return;
    };
    let _ = conn.execute(
      "INSERT OR REPLACE INTO transpilecache (specifier, source_hash, transpiled_code, \
       source_map) VALUES (?1, ?2, ?3, ?4)",
      params![specifier, source_hash as i64, transpiled_code, source_map],
    );
  }
}
