use clap::Parser;
use openraft::Raft;
use raft_kv_rocksdb::network::raft_network_impl::ExampleNetwork;
use raft_kv_rocksdb::start_example_raft_node;
use raft_kv_rocksdb::store::ExampleStore;
use raft_kv_rocksdb::ExampleTypeConfig;

pub type ExampleRaft = Raft<ExampleTypeConfig, ExampleNetwork, ExampleStore>;

#[derive(Parser, Clone, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Opt {
    #[clap(long)]
    pub id: u64,

    #[clap(long)]
    pub http_addr: String,

    #[clap(long)]
    pub rpc_addr: String,
}

#[async_std::main]
async fn main() -> std::io::Result<()> {
    // Setup the logger
    tracing_subscriber::fmt().init();

    // Parse the parameters passed by arguments.
    let options = Opt::parse();

    start_example_raft_node(
        options.id,
        format!("{}.db", options.rpc_addr),
        options.http_addr,
        options.rpc_addr,
    )
    .await
}
