extern crate clap;
use clap::{Arg, Command};
use ctrlc;
use once_cell::sync::Lazy;
use tokio_actor_joinhandle_example::actor::Actor;
use tokio_actor_joinhandle_example::adapters::{
    MemoryStorage, Multicast, OutgoingWebsocketManager, SledStorage, WsServer, WsServerConfig,
};
use tokio_actor_joinhandle_example::{Config, Node};

static DEFAULT_PORT: Lazy<String> = Lazy::new(|| WsServerConfig::default().port.to_string());

#[tokio::main]
async fn main() {
    // let default_port = WsServerConfig::default().port.to_string();
    let matches = Command::new("Rod")
        .version("1.0")
        .author("Martti Malmi")
        .about("Rod node runner")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                // .takes_value(true),
        )
        .subcommand(
            Command::new("start")
                .about("runs the rod server")
                .arg(
                    Arg::new("ws-server")
                        .long("ws-server")
                        .env("WS_SERVER")
                        .value_name("BOOL")
                        .help("Run websocket server?")
                        .default_value("true"),
                )
                .arg(
                    Arg::new("port")
                        .short('p')
                        .long("port")
                        .env("PORT")
                        .value_name("NUMBER")
                        .help("Websocket server port")
                        .default_value(&**DEFAULT_PORT),
                )
                .arg(
                    Arg::new("cert-path")
                        .long("cert-path")
                        .env("CERT_PATH")
                        .value_name("FILE")
                        .help("TLS certificate path"),
                )
                .arg(
                    Arg::new("key-path")
                        .long("key-path")
                        .env("KEY_PATH")
                        .value_name("FILE")
                        .help("TLS key path"),
                )
                .arg(
                    Arg::new("peers")
                        .long("peers")
                        .env("PEERS")
                        .value_name("URLS")
                        .help("Comma-separated outgoing websocket peers (wss://...)"),
                )
                .arg(
                    Arg::new("multicast")
                        .long("multicast")
                        .env("MULTICAST")
                        .value_name("BOOL")
                        .help("Enable multicast sync?")
                        .default_value("false"), // .takes_value(true),
                )
                .arg(
                    Arg::new("memory-storage")
                        .long("memory-storage")
                        .env("MEMORY_STORAGE")
                        .value_name("BOOL")
                        .help("In-memory storage")
                        .default_value("false"), // .takes_value(true),
                )
                .arg(
                    Arg::new("sled-storage")
                        .long("sled-storage")
                        .env("SLED_STORAGE")
                        .value_name("BOOL")
                        .help("Sled storage (disk+mem)")
                        .default_value("true"), // .takes_value(true),
                )
                .arg(
                    Arg::new("sled-max-size")
                        .long("sled-max-size")
                        .env("SLED_MAX_SIZE")
                        .value_name("BYTES")
                        .help("Data in excess of this will be evicted based on priority"), // .takes_value(true),
                )
                .arg(
                    Arg::new("allow-public-space")
                        .long("allow-public-space")
                        .env("ALLOW_PUBLIC_SPACE")
                        .value_name("BOOL")
                        .help("Allow writes that are not content hash addressed or user-signed")
                        .default_value("true"), // .takes_value(true),
                )
                .arg(
                    Arg::new("stats")
                        .long("stats")
                        .env("STATS")
                        .value_name("BOOL")
                        .help("Show stats at /stats?")
                        .default_value("true"), // .takes_value(true),
                ),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("start") {
        // TODO: write fn to convert matches into Config
        let mut outgoing_websocket_peers = Vec::new();
        if let Some(peers) = matches.get_one::<String>("peers") {
            outgoing_websocket_peers = peers.split(",").map(|s| s.to_string()).collect();
        }

        env_logger::init();

        let websocket_server_port: u16 = matches
            .get_one::<String>("port")
            .unwrap()
            .parse::<u16>()
            .unwrap();

        let sled_max_size: Option<u64> = match matches.get_one::<String>("sled-max-size") {
            Some(v) => Some(v.parse::<u64>().unwrap()),
            _ => None,
        };

        let mut network_adapters: Vec<Box<dyn Actor>> = Vec::new();
        let mut storage_adapters: Vec<Box<dyn Actor>> = Vec::new();

        let websocket_server = matches.get_one::<String>("ws-server").unwrap() == "true";

        let config = Config {
            allow_public_space: matches.get_one::<String>("allow-public-space").unwrap() != "false",
            stats: matches.get_one::<String>("stats").unwrap() == "true",
            ..Config::default()
        };

        // TODO init adapters here
        if matches.get_one::<String>("multicast").unwrap() == "true" {
            network_adapters.push(Box::new(Multicast::new(config.clone())));
        }
        if websocket_server {
            let cert_path = matches
                .get_one::<String>("cert-path")
                .map(|s| s.to_string());
            let key_path = matches.get_one::<String>("key-path").map(|s| s.to_string());
            network_adapters.push(Box::new(WsServer::new_with_config(
                config.clone(),
                WsServerConfig {
                    port: websocket_server_port,
                    cert_path,
                    key_path,
                },
            )));
        }
        if matches.get_one::<String>("sled-storage").unwrap() != "false" {
            storage_adapters.push(Box::new(SledStorage::new_with_config(
                config.clone(),
                sled::Config::default().path("sled_db"),
                sled_max_size,
            )));
        }
        if matches.get_one::<String>("memory-storage").unwrap() == "true" {
            storage_adapters.push(Box::new(MemoryStorage::new()));
        }
        if outgoing_websocket_peers.len() > 0 {
            network_adapters.push(Box::new(OutgoingWebsocketManager::new(
                config.clone(),
                outgoing_websocket_peers,
            )));
        }

        let node = Node::new_with_config(config, storage_adapters, network_adapters);

        println!("Rod node starting...");

        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();

        let mut node_clone = node.clone();
        let tx_mutex = std::sync::Mutex::new(Some(cancel_tx));
        ctrlc::set_handler(move || {
            node_clone.stop();
            if let Some(tx) = tx_mutex.lock().unwrap().take() {
                let _ = tx.send(()).unwrap();
            }
        })
        .expect("Error setting Ctrl-C handler");

        let _ = cancel_rx.await;
    }
}
