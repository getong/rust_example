use std::{collections::HashMap, str::FromStr};

use anyhow::Result;
use futures::StreamExt;
use sui_sdk::{
  rpc_types::{EventFilter, SuiEvent, SuiObjectDataOptions},
  types::{base_types::ObjectID, Identifier},
  SuiClient, SuiClientBuilder,
};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt()
    .with_env_filter("sui_event_example=debug,sui_sdk=info")
    .init();

  let example_package_id = "0x2";
  let example_object_id = "0x5";

  let mut listener = SuiEventListener::new().await?;

  listener
    .listen_to_events(example_package_id, "coin", Some(example_object_id))
    .await?;

  Ok(())
}

struct SuiEventListener {
  client: SuiClient,
  object_versions: HashMap<ObjectID, u64>,
}

impl SuiEventListener {
  async fn new() -> Result<Self> {
    let client = SuiClientBuilder::default()
      .build("https://fullnode.testnet.sui.io:443")
      .await?;

    Ok(Self {
      client,
      object_versions: HashMap::new(),
    })
  }

  async fn listen_to_events(
    &mut self,
    package_id: &str,
    module_name: &str,
    object_id: Option<&str>,
  ) -> Result<()> {
    info!(
      "Starting event listener for package: {} module: {}",
      package_id, module_name
    );
    if let Some(obj_id) = object_id {
      info!("Tracking object ID: {}", obj_id);
    }

    let package_filter = EventFilter::MoveModule {
      package: ObjectID::from_str(package_id)?,
      module: Identifier::new(module_name)?,
    };

    let mut event_stream = self
      .client
      .event_api()
      .subscribe_event(package_filter)
      .await?;

    while let Some(event) = event_stream.next().await {
      match event {
        Ok(sui_event) => {
          info!("Received event: {:#?}", sui_event);

          if let Some(obj_id_str) = object_id {
            self.process_object_changes(&sui_event, obj_id_str).await?;
          }

          if let Some(created_objects) = sui_event.parsed_json.get("created_objects") {
            info!("New objects created: {}", created_objects);
          }

          if let Some(deleted_objects) = sui_event.parsed_json.get("deleted_objects") {
            info!("Objects deleted: {}", deleted_objects);
          }

          if let Some(mutated_objects) = sui_event.parsed_json.get("mutated_objects") {
            info!("Objects mutated: {}", mutated_objects);
          }
        }
        Err(e) => {
          error!("Error receiving event: {:?}", e);
        }
      }
    }

    Ok(())
  }

  async fn process_object_changes(
    &mut self,
    _event: &SuiEvent,
    tracked_object_id: &str,
  ) -> Result<()> {
    let object_id = ObjectID::from_str(tracked_object_id)?;

    if let Ok(object) = self
      .client
      .read_api()
      .get_object_with_options(object_id, SuiObjectDataOptions::full_content())
      .await
    {
      if let Some(object_data) = object.data {
        let current_version = object_data.version.value();

        if let Some(&previous_version) = self.object_versions.get(&object_id) {
          if current_version != previous_version {
            info!(
              "Object {} changed! Version: {} -> {}",
              tracked_object_id, previous_version, current_version
            );

            if let Some(content) = &object_data.content {
              info!("New object content: {:#?}", content);
            }
          }
        } else {
          info!(
            "First time seeing object {}. Version: {}",
            tracked_object_id, current_version
          );
        }

        self.object_versions.insert(object_id, current_version);
      }
    }

    Ok(())
  }
}
