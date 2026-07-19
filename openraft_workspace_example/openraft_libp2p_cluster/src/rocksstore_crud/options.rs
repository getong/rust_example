use std::sync::LazyLock;

use rocksdb::{BlockBasedOptions, Cache, ColumnFamilyDescriptor, CompactionPri, Options};

const MIB: usize = 1024 * 1024;
const MAX_OPEN_FILES: i32 = 8_000;
const MAX_TOTAL_WAL_SIZE: u64 = 256 * MIB as u64;
const WRITE_BUFFER_SIZE: usize = 64 * MIB;
const MAX_BYTES_FOR_LEVEL_BASE: u64 = 256 * MIB as u64;
const PRIMARY_POINT_CACHE_SIZE: usize = 256 * MIB;
const PRIMARY_LOG_CACHE_SIZE: usize = 8 * MIB;
const SECONDARY_POINT_CACHE_SIZE: usize = 128 * MIB;
const SECONDARY_LOG_CACHE_SIZE: usize = 4 * MIB;

// A process can host several raft groups, each with its own DB. Sharing the
// caches keeps these values process-wide bounds instead of multiplying them
// by the number of groups.
static PRIMARY_POINT_CACHE: LazyLock<Cache> =
  LazyLock::new(|| Cache::new_lru_cache(PRIMARY_POINT_CACHE_SIZE));
static PRIMARY_LOG_CACHE: LazyLock<Cache> =
  LazyLock::new(|| Cache::new_lru_cache(PRIMARY_LOG_CACHE_SIZE));
static SECONDARY_POINT_CACHE: LazyLock<Cache> =
  LazyLock::new(|| Cache::new_lru_cache(SECONDARY_POINT_CACHE_SIZE));
static SECONDARY_LOG_CACHE: LazyLock<Cache> =
  LazyLock::new(|| Cache::new_lru_cache(SECONDARY_LOG_CACHE_SIZE));

pub(crate) fn primary_db_options() -> Options {
  let mut opts = common_db_options();
  opts.create_missing_column_families(true);
  opts.create_if_missing(true);
  opts.set_max_total_wal_size(MAX_TOTAL_WAL_SIZE);
  opts
}

pub(crate) fn secondary_db_options() -> Options {
  common_db_options()
}

pub(crate) fn primary_cf_descriptors() -> Vec<ColumnFamilyDescriptor> {
  cf_descriptors(&PRIMARY_POINT_CACHE, &PRIMARY_LOG_CACHE)
}

pub(crate) fn secondary_cf_descriptors() -> Vec<ColumnFamilyDescriptor> {
  cf_descriptors(&SECONDARY_POINT_CACHE, &SECONDARY_LOG_CACHE)
}

fn common_db_options() -> Options {
  let mut opts = Options::default();
  opts.set_max_open_files(MAX_OPEN_FILES);
  opts.set_compaction_readahead_size(2 * MIB);
  opts
}

fn cf_descriptors(point_cache: &Cache, log_cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
  vec![
    ColumnFamilyDescriptor::new("meta", point_lookup_options(point_cache)),
    ColumnFamilyDescriptor::new("sm_meta", point_lookup_options(point_cache)),
    ColumnFamilyDescriptor::new("sm_data", write_heavy_options(point_cache, false)),
    ColumnFamilyDescriptor::new("logs", write_heavy_options(log_cache, true)),
  ]
}

fn point_lookup_options(cache: &Cache) -> Options {
  let mut table = BlockBasedOptions::default();
  table.set_block_cache(cache);
  // Full filters are effective for point reads without imposing a prefix
  // contract on variable-length state-machine keys or ordered log scans.
  table.set_bloom_filter(10.0, false);
  table.set_cache_index_and_filter_blocks(true);
  table.set_pin_l0_filter_and_index_blocks_in_cache(true);

  let mut opts = Options::default();
  opts.set_block_based_table_factory(&table);
  opts
}

fn write_heavy_options(cache: &Cache, is_log: bool) -> Options {
  let mut opts = point_lookup_options(cache);
  opts.set_write_buffer_size(WRITE_BUFFER_SIZE);
  opts.set_max_write_buffer_number(4);
  opts.set_min_write_buffer_number_to_merge(2);
  opts.set_level_zero_file_num_compaction_trigger(4);
  opts.set_level_zero_slowdown_writes_trigger(20);
  opts.set_level_zero_stop_writes_trigger(40);
  opts.set_max_bytes_for_level_base(MAX_BYTES_FOR_LEVEL_BASE);
  if is_log {
    opts.set_compaction_pri(CompactionPri::MinOverlappingRatio);
  }
  opts
}
