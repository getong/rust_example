use std::{collections::HashMap, str::FromStr, time::Duration};

use anyhow::Result;
use sui_sdk::{
  rpc_types::{EventFilter, SuiEvent, SuiObjectDataOptions},
  types::{base_types::ObjectID, Identifier},
  SuiClient, SuiClientBuilder,
};
use sui_types::event::EventID;
use tokio::time;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt()
    .with_env_filter("polling_example=debug,sui_sdk=info")
    .init();

  let example_package_id = "0x2";
  let example_object_id = "0x5";

  let mut listener = SuiEventPoller::new().await?;

  listener
    .poll_events(example_package_id, "coin", Some(example_object_id))
    .await?;

  Ok(())
}

struct SuiEventPoller {
  client: SuiClient,
  object_versions: HashMap<ObjectID, u64>,
  last_cursor: Option<EventID>,
}

impl SuiEventPoller {
  async fn new() -> Result<Self> {
    let client = SuiClientBuilder::default()
      .build("https://fullnode.mainnet.sui.io:443")
      .await?;

    Ok(Self {
      client,
      object_versions: HashMap::new(),
      last_cursor: None,
    })
  }

  async fn poll_events(
    &mut self,
    package_id: &str,
    module_name: &str,
    object_id: Option<&str>,
  ) -> Result<()> {
    info!(
      "Starting event poller for package: {} module: {}",
      package_id, module_name
    );
    if let Some(obj_id) = object_id {
      info!("Tracking object ID: {}", obj_id);
    }

    let package_filter = EventFilter::MoveModule {
      package: ObjectID::from_str(package_id)?,
      module: Identifier::new(module_name)?,
    };

    let mut interval = time::interval(Duration::from_secs(5));

    loop {
      interval.tick().await;

      match self
        .client
        .event_api()
        .query_events(
          package_filter.clone(),
          self.last_cursor.clone(),
          Some(50), // limit
          false,    // descending
        )
        .await
      {
        Ok(events_page) => {
          if !events_page.data.is_empty() {
            info!("Found {} new events", events_page.data.len());

            for event in &events_page.data {
              self.process_event(event, object_id).await?;
            }

            // Update cursor for next query
            if let Some(last_event) = events_page.data.last() {
              self.last_cursor = Some(last_event.id.clone());
            }
          } else {
            info!("No new events found");
          }
        }
        Err(e) => {
          error!("Error querying events: {:?}", e);
        }
      }
    }
  }

  async fn process_event(
    &mut self,
    event: &SuiEvent,
    tracked_object_id: Option<&str>,
  ) -> Result<()> {
    info!("Processing event: {:#?}", event);

    if let Some(obj_id_str) = tracked_object_id {
      self.process_object_changes(event, obj_id_str).await?;
    }

    // Process event data
    if let Some(created_objects) = event.parsed_json.get("created_objects") {
      info!("New objects created: {}", created_objects);
    }

    if let Some(deleted_objects) = event.parsed_json.get("deleted_objects") {
      info!("Objects deleted: {}", deleted_objects);
    }

    if let Some(mutated_objects) = event.parsed_json.get("mutated_objects") {
      info!("Objects mutated: {}", mutated_objects);
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
