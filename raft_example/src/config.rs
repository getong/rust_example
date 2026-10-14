use std::{
  collections::BTreeSet,
  fs,
  io::Write,
  path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use libp2p::{Multiaddr, PeerId, identity::Keypair};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Peer {
  pub id: u64,
  pub peer_id: String,
  pub address: String,
}

#[derive(Serialize, Deserialize)]
pub struct NodeConfig {
  pub id: u64,
  pub listen: String,
  // 相对路径以配置文件目录为基准；兼容旧配置。
  #[serde(default)]
  pub data_dir: Option<PathBuf>,
  // 仅自己的私钥；其他节点的配置中只包含此节点的公钥身份。
  pub identity: Vec<u8>,
  pub peers: Vec<Peer>,
}

impl NodeConfig {
  pub fn load(path: &Path) -> Result<(Self, Keypair)> {
    let config: Self = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
      config.peers.len() == 3,
      "this example requires exactly three voters"
    );
    let key = Keypair::from_protobuf_encoding(&config.identity)?;
    let mut ids = BTreeSet::new();
    let mut peer_ids = BTreeSet::new();
    for peer in &config.peers {
      ensure!(
        peer.id != 0 && ids.insert(peer.id),
        "duplicate or zero Raft ID"
      );
      let peer_id: PeerId = peer.peer_id.parse()?;
      ensure!(peer_ids.insert(peer_id), "duplicate PeerId");
      validate_address(&peer.address)?;
    }
    let own = config
      .peers
      .iter()
      .find(|peer| peer.id == config.id)
      .context("own ID is missing from membership")?;
    ensure!(
      own.peer_id == key.public().to_peer_id().to_string(),
      "private key does not match own PeerId"
    );
    validate_address(&config.listen)?;
    Ok((config, key))
  }
}

fn validate_address(address: &str) -> Result<()> {
  use libp2p::multiaddr::Protocol;
  let address: Multiaddr = address.parse()?;
  let parts: Vec<_> = address.iter().collect();
  ensure!(
    matches!(
      parts.as_slice(),
      [Protocol::Ip4(_), Protocol::Tcp(1 ..)]
        | [Protocol::Ip6(_), Protocol::Tcp(1 ..)]
        | [Protocol::Dns(_), Protocol::Tcp(1 ..)]
        | [Protocol::Dns4(_), Protocol::Tcp(1 ..)]
        | [Protocol::Dns6(_), Protocol::Tcp(1 ..)]
    ),
    "expected /ip4|ip6|dns/HOST/tcp/PORT (nonzero port)"
  );
  Ok(())
}

pub fn init(directory: &Path, addresses: Vec<String>) -> Result<()> {
  ensure!(
    addresses.len() == 3,
    "provide exactly three advertised addresses"
  );
  for address in &addresses {
    validate_address(address)?;
  }
  ensure!(
    addresses.iter().collect::<BTreeSet<_>>().len() == 3,
    "addresses must be distinct"
  );
  let keys: Vec<_> = (0 .. 3).map(|_| Keypair::generate_ed25519()).collect();
  let peers: Vec<_> = keys
    .iter()
    .zip(&addresses)
    .enumerate()
    .map(|(index, (key, address))| Peer {
      id: index as u64 + 1,
      peer_id: key.public().to_peer_id().to_string(),
      address: address.clone(),
    })
    .collect();
  fs::create_dir_all(directory)?;
  // 预先检查，防止误覆盖现有身份。
  for id in 1 ..= 3 {
    ensure!(
      !directory.join(format!("node{id}.json")).exists(),
      "configuration already exists"
    );
  }
  for (index, key) in keys.iter().enumerate() {
    let id = index as u64 + 1;
    let config = NodeConfig {
      id,
      listen: addresses[index].clone(),
      data_dir: Some(PathBuf::from(format!("node{id}.rocksdb"))),
      identity: key.to_protobuf_encoding()?,
      peers: peers.clone(),
    };
    let path = directory.join(format!("node{id}.json"));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
      use std::os::unix::fs::OpenOptionsExt;
      options.mode(0o600);
    }
    options
      .open(&path)?
      .write_all(&serde_json::to_vec_pretty(&config)?)?;
    println!("{}", path.display());
  }
  Ok(())
}
