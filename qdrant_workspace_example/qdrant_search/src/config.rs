use std::{env, io};

#[derive(Debug)]
pub(crate) struct Config {
  pub url: String,
  pub collection: String,
  pub cluster: bool,
  pub shards: u32,
  pub replicas: u32,
  pub write_consistency: u32,
}

impl Config {
  pub fn from_env() -> io::Result<Self> {
    Self::from_lookup(|key| match env::var(key) {
      Ok(value) => Ok(Some(value)),
      Err(env::VarError::NotPresent) => Ok(None),
      Err(error) => Err(io::Error::new(io::ErrorKind::InvalidInput, error)),
    })
  }

  fn from_lookup(mut get: impl FnMut(&str) -> io::Result<Option<String>>) -> io::Result<Self> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidInput, message);
    let cluster = match get("QDRANT_CLUSTER")?.as_deref() {
      None | Some("false" | "0") => false,
      Some("true" | "1") => true,
      Some(_) => return Err(invalid("QDRANT_CLUSTER must be true, false, 1 or 0")),
    };
    let url = get("QDRANT_URL")?.unwrap_or_else(|| {
      if cluster {
        "http://localhost:16334"
      } else {
        "http://localhost:6334"
      }
      .into()
    });
    let collection = get("QDRANT_COLLECTION")?
      .unwrap_or_else(|| if cluster { "cluster_test" } else { "test" }.into());
    if collection.trim().is_empty() {
      return Err(invalid("QDRANT_COLLECTION must not be empty"));
    }
    let mut positive = |key, default| -> io::Result<u32> {
      match get(key)? {
        None => Ok(default),
        Some(value) => value.parse::<u32>().ok().filter(|n| *n > 0).ok_or_else(|| {
          io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{key} must be a positive integer"),
          )
        }),
      }
    };
    let shards = positive("QDRANT_SHARD_NUMBER", if cluster { 6 } else { 1 })?;
    let replicas = positive("QDRANT_REPLICATION_FACTOR", if cluster { 2 } else { 1 })?;
    let write_consistency = positive("QDRANT_WRITE_CONSISTENCY_FACTOR", 1)?;
    if write_consistency > replicas {
      return Err(invalid(
        "QDRANT_WRITE_CONSISTENCY_FACTOR must not exceed QDRANT_REPLICATION_FACTOR",
      ));
    }
    if !cluster && replicas > 1 {
      return Err(invalid("replication requires QDRANT_CLUSTER=true"));
    }
    Ok(Self {
      url,
      collection,
      cluster,
      shards,
      replicas,
      write_consistency,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn config(values: &[(&str, &str)]) -> io::Result<Config> {
    Config::from_lookup(|key| {
      Ok(
        values
          .iter()
          .find(|(k, _)| *k == key)
          .map(|(_, v)| (*v).into()),
      )
    })
  }

  #[test]
  fn single_node_defaults_remain_compatible() {
    let c = config(&[]).unwrap();
    assert!(!c.cluster);
    assert_eq!((c.shards, c.replicas, c.write_consistency), (1, 1, 1));
    assert_eq!(c.url, "http://localhost:6334");
    assert_eq!(c.collection, "test");
  }

  #[test]
  fn cluster_defaults_and_overrides() {
    let c = config(&[("QDRANT_CLUSTER", "true")]).unwrap();
    assert_eq!((c.shards, c.replicas, c.write_consistency), (6, 2, 1));
    assert_eq!(c.url, "http://localhost:16334");
    assert_eq!(c.collection, "cluster_test");
    let c = config(&[
      ("QDRANT_CLUSTER", "1"),
      ("QDRANT_URL", "http://localhost:16344"),
      ("QDRANT_SHARD_NUMBER", "12"),
      ("QDRANT_REPLICATION_FACTOR", "3"),
      ("QDRANT_WRITE_CONSISTENCY_FACTOR", "2"),
      ("QDRANT_COLLECTION", "custom"),
    ])
    .unwrap();
    assert_eq!((c.shards, c.replicas, c.write_consistency), (12, 3, 2));
    assert_eq!(c.url, "http://localhost:16344");
    assert_eq!(c.collection, "custom");
  }

  #[test]
  fn rejects_invalid_settings_before_connecting() {
    for pair in [
      ("QDRANT_CLUSTER", "yes"),
      ("QDRANT_SHARD_NUMBER", "0"),
      ("QDRANT_SHARD_NUMBER", "-1"),
      ("QDRANT_REPLICATION_FACTOR", "bad"),
      ("QDRANT_WRITE_CONSISTENCY_FACTOR", "0"),
      ("QDRANT_COLLECTION", " "),
      ("QDRANT_WRITE_CONSISTENCY_FACTOR", "3"),
    ] {
      assert!(config(&[pair]).is_err(), "accepted {pair:?}");
    }
    assert!(config(&[("QDRANT_REPLICATION_FACTOR", "2")]).is_err());
  }
}
