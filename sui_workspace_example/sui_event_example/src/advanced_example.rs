use std::{
  collections::HashMap,
  str::FromStr,
  time::{Duration, Instant},
};

use anyhow::Result;
use futures::StreamExt;
use sui_sdk::{
  rpc_types::{EventFilter, SuiEvent, SuiObjectDataOptions},
  types::{base_types::ObjectID, Identifier},
  SuiClient, SuiClientBuilder,
};
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt()
    .with_env_filter("advanced_example=debug,sui_sdk=info")
    .init();

  let defi_package = "0x1eabed72c53feb3805120a081dc15963c204dc8d091542592abaf7a35689b2fb";
  let tracked_objects = vec![
    "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf",
    "0x6",
  ];

  let mut listener = AdvancedEventListener::new().await?;

  listener
    .monitor_defi_protocol(defi_package, "cetus", tracked_objects)
    .await?;

  Ok(())
}

#[derive(Debug)]
#[allow(dead_code)]
struct ObjectChange {
  object_id: ObjectID,
  old_version: u64,
  new_version: u64,
  timestamp: Instant,
  change_type: ChangeType,
  details: Option<serde_json::Value>,
}

#[derive(Debug)]
#[allow(dead_code)]
enum ChangeType {
  Created,
  Modified,
  Deleted,
  OwnerChanged,
}

struct AdvancedEventListener {
  client: SuiClient,
  object_history: HashMap<ObjectID, Vec<ObjectChange>>,
  object_versions: HashMap<ObjectID, u64>,
  event_count: u64,
}

impl AdvancedEventListener {
  async fn new() -> Result<Self> {
    let client = SuiClientBuilder::default()
      .ws_url("wss://rpc.mainnet.sui.io:443")
      .build("https://fullnode.mainnet.sui.io:443")
      .await?;

    Ok(Self {
      client,
      object_history: HashMap::new(),
      object_versions: HashMap::new(),
      event_count: 0,
    })
  }

  async fn monitor_defi_protocol(
    &mut self,
    package_id: &str,
    module_name: &str,
    tracked_objects: Vec<&str>,
  ) -> Result<()> {
    info!(
      "Starting DeFi protocol monitor for package: {} module: {}",
      package_id, module_name
    );
    info!("Tracking {} objects", tracked_objects.len());

    for obj_id in &tracked_objects {
      if let Ok(initial_state) = self.fetch_object_state(obj_id).await {
        info!("Initial state for {}: version {}", obj_id, initial_state.0);
        let object_id = ObjectID::from_str(obj_id)?;
        self.object_versions.insert(object_id, initial_state.0);
      }
    }

    let package_filter = EventFilter::MoveModule {
      package: ObjectID::from_str(package_id)?,
      module: Identifier::new(module_name)?,
    };

    let stream_client = self.client.clone();
    let mut event_stream = stream_client
      .event_api()
      .subscribe_event(package_filter)
      .await?;

    info!("Event stream connected. Listening for events...");

    while let Some(event) = event_stream.next().await {
      match event {
        Ok(sui_event) => {
          self.event_count += 1;
          self.process_event(&sui_event, &tracked_objects).await?;
        }
        Err(e) => {
          error!("Error receiving event: {:?}", e);
          tokio::time::sleep(Duration::from_secs(5)).await;
        }
      }
    }

    Ok(())
  }

  async fn process_event(&mut self, event: &SuiEvent, tracked_objects: &[&str]) -> Result<()> {
    debug!(
      "Processing event #{}: type {}",
      self.event_count, event.type_
    );

    self
      .extract_object_changes_from_event(event, tracked_objects)
      .await?;

    self.analyze_event_patterns(event)?;

    if self.event_count % 10 == 0 {
      self.print_statistics();
    }

    Ok(())
  }

  async fn extract_object_changes_from_event(
    &mut self,
    _event: &SuiEvent,
    tracked_objects: &[&str],
  ) -> Result<()> {
    for obj_id_str in tracked_objects {
      let object_id = ObjectID::from_str(obj_id_str)?;

      if let Ok((current_version, content)) = self.fetch_object_state(obj_id_str).await {
        if let Some(&previous_version) = self.object_versions.get(&object_id) {
          if current_version != previous_version {
            let change = ObjectChange {
              object_id,
              old_version: previous_version,
              new_version: current_version,
              timestamp: Instant::now(),
              change_type: ChangeType::Modified,
              details: content,
            };

            info!(
              "🔄 Object {} changed: v{} → v{}",
              obj_id_str, previous_version, current_version
            );

            self
              .object_history
              .entry(object_id)
              .or_insert_with(Vec::new)
              .push(change);

            self.object_versions.insert(object_id, current_version);
          }
        }
      }
    }

    Ok(())
  }

  async fn fetch_object_state(&self, object_id: &str) -> Result<(u64, Option<serde_json::Value>)> {
    let object_id = ObjectID::from_str(object_id)?;
    let object = self
      .client
      .read_api()
      .get_object_with_options(object_id, SuiObjectDataOptions::full_content())
      .await?;

    if let Some(object_data) = object.data {
      let version = object_data.version.value();
      let content = object_data
        .content
        .map(|c| serde_json::to_value(&c).unwrap_or(serde_json::Value::Null));
      Ok((version, content))
    } else {
      Err(anyhow::anyhow!("Object not found"))
    }
  }

  fn analyze_event_patterns(&self, event: &SuiEvent) -> Result<()> {
    let type_str = event.type_.to_string();

    if type_str.contains("Swap") {
      info!("💱 Swap event detected: {}", event.type_);
      if let Some(amount_in) = event.parsed_json.get("amount_in") {
        info!("  Amount in: {}", amount_in);
      }
      if let Some(amount_out) = event.parsed_json.get("amount_out") {
        info!("  Amount out: {}", amount_out);
      }
    }

    if type_str.contains("Deposit") || type_str.contains("Withdraw") {
      info!("💰 Liquidity event: {}", event.type_);
    }

    if type_str.contains("Transfer") {
      if let Some(recipient) = event.parsed_json.get("recipient") {
        info!("📤 Transfer to: {}", recipient);
      }
    }

    Ok(())
  }

  fn print_statistics(&self) {
    info!("📊 Event Statistics:");
    info!("  Total events processed: {}", self.event_count);
    info!("  Objects being tracked: {}", self.object_versions.len());

    for (object_id, changes) in &self.object_history {
      if !changes.is_empty() {
        info!("  Object {}: {} changes", object_id, changes.len());
        if let Some(last_change) = changes.last() {
          info!("    Last change: {:?} ago", last_change.timestamp.elapsed());
        }
      }
    }
  }
}
