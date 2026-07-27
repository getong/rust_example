mod rocksdb_task_repository;

use std::path::PathBuf;

pub use rocksdb_task_repository::RocksDbTaskRepository;

#[must_use]
pub fn default_database_path() -> PathBuf {
  dirs::data_local_dir()
    .unwrap_or_else(std::env::temp_dir)
    .join("dioxus-clean-architecture")
    .join("tasks.rocksdb")
}
