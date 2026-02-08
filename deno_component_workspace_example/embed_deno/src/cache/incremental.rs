// Copyright 2018-2026 the Deno authors. MIT license.

use super::cache_db::{CacheDBConfiguration, CacheFailure};

// This fork no longer uses Deno CLI's incremental cache subsystem (it was
// primarily used by formatting/linting tooling, which is removed).
//
// Keep only the sqlite DB schema descriptor so the rest of the cache plumbing
// continues to compile without pulling in unused logic.

pub static INCREMENTAL_CACHE_DB: CacheDBConfiguration = CacheDBConfiguration {
  table_initializer: "",
  on_version_change: "",
  preheat_queries: &[],
  on_failure: CacheFailure::Blackhole,
};
