mod config;

use std::io;

use config::Config;
use qdrant_client::{
  Payload, Qdrant, QdrantError,
  qdrant::{
    Condition, CreateCollectionBuilder, Distance, Filter, PointStruct, ScalarQuantizationBuilder,
    SearchParamsBuilder, SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
  },
};

#[tokio::main]
async fn main() -> Result<(), QdrantError> {
  // Example of top level client
  // You may also use tonic-generated client from `src/qdrant.rs`
  let config = Config::from_env()?;
  let client = Qdrant::from_url(&config.url).build()?;

  let collections_list = client.list_collections().await?;
  dbg!(collections_list);
  // collections_list = {
  //   "collections": [
  //     {
  //       "name": "test"
  //     }
  //   ]
  // }

  let collection_name = config.collection.as_str();

  if !client.collection_exists(collection_name).await? {
    client
      .create_collection(
        CreateCollectionBuilder::new(collection_name)
          .shard_number(config.shards)
          .replication_factor(config.replicas)
          .write_consistency_factor(config.write_consistency)
          .vectors_config(VectorParamsBuilder::new(10, Distance::Cosine))
          .quantization_config(ScalarQuantizationBuilder::default()),
      )
      .await?;
  }

  let collection_info = client.collection_info(collection_name).await?;
  let params = collection_info
    .result
    .as_ref()
    .and_then(|info| info.config.as_ref())
    .and_then(|config| config.params.as_ref())
    .ok_or_else(|| io::Error::other("collection parameters missing from server response"))?;
  if params.shard_number != config.shards
    || params.replication_factor.unwrap_or(1) != config.replicas
    || params.write_consistency_factor.unwrap_or(1) != config.write_consistency
  {
    return Err(
      io::Error::other(format!(
        "collection {collection_name} has different shard/replication settings; use a new \
         QDRANT_COLLECTION or match its existing settings"
      ))
      .into(),
    );
  }
  dbg!(collection_info);
  if config.cluster {
    let cluster_info = client.collection_cluster_info(collection_name).await?;
    dbg!(cluster_info);
  }

  let payload: Payload = serde_json::json!(
    {
      "foo": "Bar",
      "bar": 12,
      "baz": {
        "qux": "quux"
      }
    }
  )
  .try_into()?;

  let points = vec![PointStruct::new(0, vec![12.; 10], payload)];
  client
    .upsert_points(UpsertPointsBuilder::new(collection_name, points).wait(true))
    .await?;

  let search_result = client
    .search_points(
      SearchPointsBuilder::new(collection_name, [11.; 10], 10)
        .filter(Filter::all([Condition::matches("bar", 12)]))
        .with_payload(true)
        .params(SearchParamsBuilder::default().exact(true)),
    )
    .await?;
  dbg!(&search_result);
  // search_result = [
  //   {
  //     "id": 0,
  //     "version": 0,
  //     "score": 1.0000001,
  //     "payload": {
  //       "bar": 12,
  //       "baz": {
  //         "qux": "quux"
  //       },
  //       "foo": "Bar"
  //     }
  //   }
  // ]

  let found_point = search_result
    .result
    .into_iter()
    .next()
    .ok_or_else(|| io::Error::other("search returned no points"))?;
  let mut payload = found_point.payload;
  let baz_payload = payload
    .remove("baz")
    .ok_or_else(|| io::Error::other("search result is missing baz payload"))?
    .into_json();
  println!("baz: {baz_payload}");
  // baz: {"qux":"quux"}

  Ok(())
}

// copy from https://github.com/qdrant/rust-client/blob/master/examples/search.rs
