use std::collections::HashSet;

use anyhow::Result;
use libp2p::{
  // core::transport::upgrade::Version,
  floodsub::{Floodsub, FloodsubEvent, Topic},
  futures::StreamExt,
  identity,
  identity::Keypair,
  mdns,
  noise,
  swarm::{NetworkBehaviour, Swarm},
  tcp,
  yamux,
  PeerId,
  SwarmBuilder,
};
use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::{fs, io::AsyncBufReadExt, time::Duration};
// use std::io::Result;
// use tokio::io::{AsyncRead, AsyncWrite};

const STORAGE_FILE_PATH: &str = "./recipes.json";

type RecipeResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

type Recipes = Vec<Recipe>;

static KEYS: Lazy<identity::Keypair> = Lazy::new(identity::Keypair::generate_ed25519);
static PEER_ID: Lazy<PeerId> = Lazy::new(|| PeerId::from(KEYS.public()));
static TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("recipes"));

#[derive(Debug, Serialize, Deserialize)]
struct Recipe {
  id: usize,
  name: String,
  ingredients: String,
  instructions: String,
  public: bool,
}

#[derive(Debug, Serialize, Deserialize)]
enum ListMode {
  ALL,
  One(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct ListRequest {
  mode: ListMode,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListResponse {
  mode: ListMode,
  data: Recipes,
  receiver: String,
}

#[derive(Debug)]
enum EventType {
  #[allow(dead_code)]
  Response(ListResponse),
  Input(String),
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "RecipeBehaviourEvent")]
struct RecipeBehaviour {
  floodsub: Floodsub,
  mdns: mdns::tokio::Behaviour,
  // #[behaviour(ignore)]
  // response_sender: mpsc::UnboundedSender<ListResponse>,
}

#[derive(Debug)]
pub enum RecipeBehaviourEvent {
  FloodSub(FloodsubEvent),
  Mdns(mdns::Event),
}

impl From<FloodsubEvent> for RecipeBehaviourEvent {
  fn from(event: FloodsubEvent) -> Self {
    RecipeBehaviourEvent::FloodSub(event)
  }
}

impl From<mdns::Event> for RecipeBehaviourEvent {
  fn from(event: mdns::Event) -> Self {
    RecipeBehaviourEvent::Mdns(event)
  }
}

async fn create_new_recipe(name: &str, ingredients: &str, instructions: &str) -> RecipeResult<()> {
  let mut local_recipes = read_local_recipes().await?;
  let new_id = match local_recipes.iter().max_by_key(|r| r.id) {
    Some(v) => v.id + 1,
    None => 0,
  };
  local_recipes.push(Recipe {
    id: new_id,
    name: name.to_owned(),
    ingredients: ingredients.to_owned(),
    instructions: instructions.to_owned(),
    public: false,
  });
  write_local_recipes(&local_recipes).await?;

  info!("Created recipe:");
  info!("Name: {}", name);
  info!("Ingredients: {}", ingredients);
  info!("Instructions:: {}", instructions);

  Ok(())
}

async fn publish_recipe(id: usize) -> RecipeResult<()> {
  let mut local_recipes = read_local_recipes().await?;
  local_recipes
    .iter_mut()
    .filter(|r| r.id == id)
    .for_each(|r| r.public = true);
  write_local_recipes(&local_recipes).await?;
  Ok(())
}

async fn read_local_recipes() -> RecipeResult<Recipes> {
  let content = fs::read(STORAGE_FILE_PATH).await?;
  let result = serde_json::from_slice(&content)?;
  Ok(result)
}

async fn write_local_recipes(recipes: &Recipes) -> RecipeResult<()> {
  let json = serde_json::to_string(&recipes)?;
  fs::write(STORAGE_FILE_PATH, &json).await?;
  Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
  pretty_env_logger::init();

  info!("Peer Id: {}", PEER_ID.clone());
  // let (response_sender, mut response_rcv) = mpsc::unbounded_channel();

  // let auth_keys = Keypair::<X25519Spec>::new()
  //     .into_authentic(&KEYS)
  //     .expect("can create auth keys");
  // Generate a new keypair for our local peer
  let local_keypair = Keypair::generate_secp256k1();

  // let transp = TokioTcpConfig::new()
  //     .upgrade(upgrade::Version::V1)
  //     .authenticate(NoiseConfig::xx(auth_keys).into_authenticated()) // XX Handshake pattern, IX
  // exists as well and IK - only XX currently provides interop with other libp2p impls
  //     .multiplex(libp2p_mplex::MplexConfig::new())
  //     .boxed();
  // Create a TCP transport
  // let transp = tcp::tokio::Transport::default()
  //   .upgrade(Version::V1Lazy)
  //   .authenticate(noise::Config::new(&local_keypair).unwrap())
  //   .multiplex(yamux::Config::default())
  //   .boxed();

  // Create an identity for our local peer
  // let local_peer_id = PeerId::from_public_key(&local_keypair.public());

  // let mut behaviour = RecipeBehaviour {
  //   floodsub: Floodsub::new(*PEER_ID),
  //   mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap(),
  //   // response_sender,
  // };

  let mut swarm = SwarmBuilder::with_existing_identity(local_keypair.clone())
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_dns()?
    .with_behaviour(|key| {
      // let peer_id = PeerId::from(key.public());
      // MyBehaviour::new(peer_id).unwrap()
      let local_peer_id = PeerId::from_public_key(&key.public());
      RecipeBehaviour {
        floodsub: Floodsub::new(*PEER_ID),
        mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap(),
        // response_sender,
      }
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
    .build();

  // behaviour.floodsub.subscribe(TOPIC.clone());

  // let mut swarm = SwarmBuilder::with_tokio_executor(transp, behaviour, *PEER_ID).build();
  swarm.behaviour_mut().floodsub.subscribe(TOPIC.clone());

  let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();

  swarm
    .listen_on(
      "/ip4/0.0.0.0/tcp/0"
        .parse()
        .expect("can get a local socket"),
    )
    .expect("swarm can be started");

  loop {
    let evt = {
      tokio::select! {
          line = stdin.next_line() => Some(EventType::Input(line.expect("can get line").expect("can read line from stdin"))),
          // response = response_rcv.recv() => Some(EventType::Response(response.expect("response exists"))),
          event = swarm.select_next_some() => {
              info!("Unhandled Swarm Event: {:?}", event);
              None
          },
      }
    };

    if let Some(event) = evt {
      match event {
        EventType::Response(resp) => {
          let json = serde_json::to_string(&resp).expect("can jsonify response");
          swarm.behaviour_mut().floodsub.publish(TOPIC.clone(), json);
        }
        EventType::Input(line) => match line.as_str() {
          "ls p" => handle_list_peers(&mut swarm).await,
          cmd if cmd.starts_with("ls r") => handle_list_recipes(cmd, &mut swarm).await,
          cmd if cmd.starts_with("create r") => handle_create_recipe(cmd).await,
          cmd if cmd.starts_with("publish r") => handle_publish_recipe(cmd).await,
          _ => error!("unknown command"),
        },
      }
    }
  }
}

// async fn handle_list_peers(swarm: &mut Swarm<RecipeBehaviour<TSubstream>>) {
async fn handle_list_peers(swarm: &mut Swarm<RecipeBehaviour>) {
  info!("Discovered Peers:");
  let nodes = swarm.behaviour().mdns.discovered_nodes();
  let mut unique_peers = HashSet::new();
  for peer in nodes {
    unique_peers.insert(peer);
  }
  unique_peers.iter().for_each(|p| info!("{}", p));
}

async fn handle_list_recipes(cmd: &str, swarm: &mut Swarm<RecipeBehaviour>) {
  let rest = cmd.strip_prefix("ls r ");
  match rest {
    Some("all") => {
      let req = ListRequest {
        mode: ListMode::ALL,
      };
      let json = serde_json::to_string(&req).expect("can jsonify request");
      swarm.behaviour_mut().floodsub.publish(TOPIC.clone(), json);
    }
    Some(recipes_peer_id) => {
      let req = ListRequest {
        mode: ListMode::One(recipes_peer_id.to_owned()),
      };
      let json = serde_json::to_string(&req).expect("can jsonify request");
      swarm.behaviour_mut().floodsub.publish(TOPIC.clone(), json);
    }
    None => {
      match read_local_recipes().await {
        Ok(v) => {
          info!("Local Recipes ({})", v.len());
          v.iter().for_each(|r| info!("{:?}", r));
        }
        Err(e) => error!("error fetching local recipes: {}", e),
      };
    }
  };
}

async fn handle_create_recipe(cmd: &str) {
  if let Some(rest) = cmd.strip_prefix("create r") {
    let elements: Vec<&str> = rest.split('|').collect();
    if elements.len() < 3 {
      info!("too few arguments - Format: name|ingredients|instructions");
    } else {
      let name = elements.first().expect("name is there");
      let ingredients = elements.get(1).expect("ingredients is there");
      let instructions = elements.get(2).expect("instructions is there");
      if let Err(e) = create_new_recipe(name, ingredients, instructions).await {
        error!("error creating recipe: {}", e);
      };
    }
  }
}

async fn handle_publish_recipe(cmd: &str) {
  if let Some(rest) = cmd.strip_prefix("publish r") {
    match rest.trim().parse::<usize>() {
      Ok(id) => {
        if let Err(e) = publish_recipe(id).await {
          info!("error publishing recipe with id {}, {}", id, e)
        } else {
          info!("Published Recipe with id: {}", id);
        }
      }
      Err(e) => error!("invalid id: {}, {}", rest.trim(), e),
    };
  }
}
