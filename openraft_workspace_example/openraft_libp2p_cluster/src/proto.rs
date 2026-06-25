pub mod raft_kv {
  include!(concat!(env!("OUT_DIR"), "/libp2p_openraft_rocksdb.rs"));
}
