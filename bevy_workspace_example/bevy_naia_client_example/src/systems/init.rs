use bevy::{
  log::info,
  prelude::{Camera2d, Commands},
};
use naia_bevy_client::{transport::webrtc, Client, DefaultClientTag};
use naia_bevy_demo_shared::messages::Auth;

use crate::resources::Global;

pub fn init(mut commands: Commands, mut client: Client<DefaultClientTag>) {
  info!("Naia Bevy Client Demo started");

  client.auth(Auth::new("charlie", "12345"));
  let socket = webrtc::Socket::new("http://127.0.0.1:14191", client.socket_config());
  client.connect(socket);

  // Setup Camera
  commands.spawn(Camera2d);

  // Setup Global Resource
  commands.insert_resource(Global::default());
}
