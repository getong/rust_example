use super::raft_types::TypeConfig;

/// Type alias for log store using mem-log crate
/// This follows the current OpenRAFT API patterns
pub type LogStore = mem_log::LogStore<TypeConfig>;
