use std::{sync::Arc, time::Duration};

use rollo::{
  error::Error,
  game::GameTime,
  packet::{to_bytes, Packet},
  server::{ListenerSecurity, SocketTools, World, WorldSession, WorldSocketMgr},
  tokio, AtomicCell,
};

#[tokio::main]
async fn main() {
  // lazy_static works as well.
  let world = Box::leak(Box::new(MyWorld {
    game_time: AtomicCell::new(GameTime::new()),
  }));

  let mut socket_manager = WorldSocketMgr::new(world);
  // Run the server and the game loop with an interval (15ms)
  socket_manager
    .start_game_loop(Duration::from_millis(15))
    .start_network("127.0.0.1:6666", ListenerSecurity::Tcp)
    .await
    .unwrap();
}

struct MyWorld {
  game_time: AtomicCell<GameTime>,
}

impl World for MyWorld {
  type WorldSessionimplementer = MyWorldSession;
  fn update(&'static self, _diff: i64, game_time: GameTime) {
    println!("Update at : {}", game_time.timestamp);
  }

  // Your GameTime will be updated automatically. (Optional)
  fn game_time(&'static self) -> Option<&'static AtomicCell<GameTime>> {
    Some(&self.game_time)
  }
}

struct MyWorldSession {
  socket_tools: SocketTools,
}

#[rollo::async_trait]
impl WorldSession<MyWorld> for MyWorldSession {
  async fn on_open(
    tools: SocketTools,
    _world: &'static MyWorld,
  ) -> Result<std::sync::Arc<Self>, Error> {
    Ok(Arc::new(Self {
      socket_tools: tools,
    }))
  }

  fn socket_tools(&self) -> &SocketTools {
    &self.socket_tools
  }

  async fn on_message(world_session: &Arc<Self>, _world: &'static MyWorld, packet: Packet) {
    // If the message received is Login(1), send a response to the player.
    if packet.cmd == 1 {
      // Create a packet without payload
      let new_packet = to_bytes(10, None);
      let new_packet = new_packet;
      // Send it to the player
      world_session.socket_tools.send_data(new_packet.into());
    }
  }

  async fn on_close(_world_session: &Arc<Self>, _world: &'static MyWorld) {
    println!("Session closed");
  }
}
