use once_cell::sync::OnceCell;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use chitchat::transport::UdpTransport;
use chitchat::{spawn_chitchat, Chitchat, ChitchatConfig, ChitchatId, FailureDetectorConfig};

use cool_id_generator::Size;

static SHARED_DATA: OnceCell<Arc<Mutex<Chitchat>>> = OnceCell::new();

fn generate_server_id(public_addr: SocketAddr) -> String {
    let cool_id = cool_id_generator::get_id(Size::Medium);
    format!("server:{public_addr}-{cool_id}")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let public_addr: SocketAddr = "127.0.0.1:10000".parse().unwrap();
    let node_id = generate_server_id(public_addr);

    let seeds: Vec<String> = vec![];
    let chitchat_id = ChitchatId::new(node_id, 0, public_addr);
    let config = ChitchatConfig {
        cluster_id: "testing".to_string(),
        chitchat_id,
        gossip_interval: Duration::from_millis(500),
        listen_addr: public_addr,
        seed_nodes: seeds,
        failure_detector_config: FailureDetectorConfig::default(),
        marked_for_deletion_grace_period: 10_000,
    };

    let chitchat_handler = spawn_chitchat(config, Vec::new(), &UdpTransport).await?;
    let chitchat = chitchat_handler.chitchat();

    SHARED_DATA.get_or_init(|| chitchat);

    Ok(())
}
